// Copyright 2020 Parity Technologies (UK) Ltd.
// This file is part of Polkadot.

// Polkadot is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Polkadot is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Polkadot.  If not, see <http://www.gnu.org/licenses/>.

//! Implements the Runtime API Subsystem
//!
//! This provides a clean, ownerless wrapper around the parachain-related runtime APIs. This crate
//! can also be used to cache responses from heavy runtime APIs.

#![deny(unused_crate_dependencies)]
#![warn(missing_docs)]

use indracore_subsystem::{
	Subsystem, SpawnedSubsystem, SubsystemResult, SubsystemContext,
	FromOverseer, OverseerSignal,
	messages::{
		RuntimeApiMessage, RuntimeApiRequest as Request,
	},
	errors::RuntimeApiError,
};
use indracore_node_subsystem_util::metrics::{self, prometheus};
use indracore_primitives::v1::{Block, BlockId, Hash, ParachainHost};

use sp_api::ProvideRuntimeApi;
use sp_core::traits::SpawnNamed;

use futures::{prelude::*, stream::FuturesUnordered, channel::oneshot, select};
use std::{sync::Arc, collections::VecDeque, pin::Pin};

const LOG_TARGET: &str = "runtime_api";

/// The number of maximum runtime api requests can be executed in parallel. Further requests will be buffered.
const MAX_PARALLEL_REQUESTS: usize = 4;

/// The name of the blocking task that executes a runtime api request.
const API_REQUEST_TASK_NAME: &str = "indracore-runtime-api-request";

/// The `RuntimeApiSubsystem`. See module docs for more details.
pub struct RuntimeApiSubsystem<Client> {
	client: Arc<Client>,
	metrics: Metrics,
	spawn_handle: Box<dyn SpawnNamed>,
	/// If there are [`MAX_PARALLEL_REQUESTS`] requests being executed, we buffer them in here until they can be executed.
	waiting_requests: VecDeque<(Pin<Box<dyn Future<Output = ()> + Send>>, oneshot::Receiver<()>)>,
	/// All the active runtime api requests that are currently being executed.
	active_requests: FuturesUnordered<oneshot::Receiver<()>>,
}

impl<Client> RuntimeApiSubsystem<Client> {
	/// Create a new Runtime API subsystem wrapping the given client and metrics.
	pub fn new(client: Arc<Client>, metrics: Metrics, spawn_handle: impl SpawnNamed + 'static) -> Self {
		RuntimeApiSubsystem {
			client,
			metrics,
			spawn_handle: Box::new(spawn_handle),
			waiting_requests: Default::default(),
			active_requests: Default::default(),
		}
	}
}

impl<Client, Context> Subsystem<Context> for RuntimeApiSubsystem<Client> where
	Client: ProvideRuntimeApi<Block> + Send + 'static + Sync,
	Client::Api: ParachainHost<Block>,
	Context: SubsystemContext<Message = RuntimeApiMessage>
{
	fn start(self, ctx: Context) -> SpawnedSubsystem {
		SpawnedSubsystem {
			future: run(ctx, self).boxed(),
			name: "runtime-api-subsystem",
		}
	}
}

impl<Client> RuntimeApiSubsystem<Client> where
	Client: ProvideRuntimeApi<Block> + Send + 'static + Sync,
	Client::Api: ParachainHost<Block>,
{
	/// Spawn a runtime api request.
	///
	/// If there are already [`MAX_PARALLEL_REQUESTS`] requests being executed, the request will be buffered.
	fn spawn_request(&mut self, relay_parent: Hash, request: Request) {
		let client = self.client.clone();
		let metrics = self.metrics.clone();
		let (sender, receiver) = oneshot::channel();

		let request = async move {
			make_runtime_api_request(
				client,
				metrics,
				relay_parent,
				request,
			);
			let _ = sender.send(());
		}.boxed();

		if self.active_requests.len() >= MAX_PARALLEL_REQUESTS {
			self.waiting_requests.push_back((request, receiver));

			if self.waiting_requests.len() > MAX_PARALLEL_REQUESTS * 10 {
				tracing::warn!(
					target: LOG_TARGET,
					"{} runtime api requests waiting to be executed.",
					self.waiting_requests.len(),
				)
			}
		} else {
			self.spawn_handle.spawn_blocking(API_REQUEST_TASK_NAME, request);
			self.active_requests.push(receiver);
		}
	}

	/// Poll the active runtime api requests.
	async fn poll_requests(&mut self) {
		// If there are no active requests, this future should be pending forever.
		if self.active_requests.len() == 0 {
			return futures::pending!()
		}

		// If there are active requests, this will always resolve to `Some(_)` when a request is finished.
		let _ = self.active_requests.next().await;

		if let Some((req, recv)) = self.waiting_requests.pop_front() {
			self.spawn_handle.spawn_blocking(API_REQUEST_TASK_NAME, req);
			self.active_requests.push(recv);
		}
	}
}

#[tracing::instrument(skip(ctx, subsystem), fields(subsystem = LOG_TARGET))]
async fn run<Client>(
	mut ctx: impl SubsystemContext<Message = RuntimeApiMessage>,
	mut subsystem: RuntimeApiSubsystem<Client>,
) -> SubsystemResult<()> where
	Client: ProvideRuntimeApi<Block> + Send + Sync + 'static,
	Client::Api: ParachainHost<Block>,
{
	loop {
		select! {
			req = ctx.recv().fuse() => match req? {
				FromOverseer::Signal(OverseerSignal::Conclude) => return Ok(()),
				FromOverseer::Signal(OverseerSignal::ActiveLeaves(_)) => {},
				FromOverseer::Signal(OverseerSignal::BlockFinalized(_)) => {},
				FromOverseer::Communication { msg } => match msg {
					RuntimeApiMessage::Request(relay_parent, request) => {
						subsystem.spawn_request(relay_parent, request);
					},
				}
			},
			_ = subsystem.poll_requests().fuse() => {},
		}
	}
}

#[tracing::instrument(level = "trace", skip(client, metrics), fields(subsystem = LOG_TARGET))]
fn make_runtime_api_request<Client>(
	client: Arc<Client>,
	metrics: Metrics,
	relay_parent: Hash,
	request: Request,
) where
	Client: ProvideRuntimeApi<Block>,
	Client::Api: ParachainHost<Block>,
{
	let _timer = metrics.time_make_runtime_api_request();

	macro_rules! query {
		($api_name:ident ($($param:expr),*), $sender:expr) => {{
			let sender = $sender;
			let api = client.runtime_api();
			let res = api.$api_name(&BlockId::Hash(relay_parent), $($param),*)
				.map_err(|e| RuntimeApiError::from(format!("{:?}", e)));
			metrics.on_request(res.is_ok());
			let _ = sender.send(res);
		}}
	}

	match request {
		Request::Validators(sender) => query!(validators(), sender),
		Request::ValidatorGroups(sender) => query!(validator_groups(), sender),
		Request::AvailabilityCores(sender) => query!(availability_cores(), sender),
		Request::PersistedValidationData(para, assumption, sender) =>
			query!(persisted_validation_data(para, assumption), sender),
		Request::FullValidationData(para, assumption, sender) =>
			query!(full_validation_data(para, assumption), sender),
		Request::CheckValidationOutputs(para, commitments, sender) =>
			query!(check_validation_outputs(para, commitments), sender),
		Request::SessionIndexForChild(sender) => query!(session_index_for_child(), sender),
		Request::ValidationCode(para, assumption, sender) =>
			query!(validation_code(para, assumption), sender),
		Request::HistoricalValidationCode(para, at, sender) =>
			query!(historical_validation_code(para, at), sender),
		Request::CandidatePendingAvailability(para, sender) =>
			query!(candidate_pending_availability(para), sender),
		Request::CandidateEvents(sender) => query!(candidate_events(), sender),
		Request::SessionInfo(index, sender) => query!(session_info(index), sender),
		Request::DmqContents(id, sender) => query!(dmq_contents(id), sender),
		Request::InboundHrmpChannelsContents(id, sender) => query!(inbound_hrmp_channels_contents(id), sender),
	}
}

#[derive(Clone)]
struct MetricsInner {
	chain_api_requests: prometheus::CounterVec<prometheus::U64>,
	make_runtime_api_request: prometheus::Histogram,
}

/// Runtime API metrics.
#[derive(Default, Clone)]
pub struct Metrics(Option<MetricsInner>);

impl Metrics {
	fn on_request(&self, succeeded: bool) {
		if let Some(metrics) = &self.0 {
			if succeeded {
				metrics.chain_api_requests.with_label_values(&["succeeded"]).inc();
			} else {
				metrics.chain_api_requests.with_label_values(&["failed"]).inc();
			}
		}
	}

	/// Provide a timer for `make_runtime_api_request` which observes on drop.
	fn time_make_runtime_api_request(&self) -> Option<metrics::prometheus::prometheus::HistogramTimer> {
		self.0.as_ref().map(|metrics| metrics.make_runtime_api_request.start_timer())
	}
}

impl metrics::Metrics for Metrics {
	fn try_register(registry: &prometheus::Registry) -> Result<Self, prometheus::PrometheusError> {
		let metrics = MetricsInner {
			chain_api_requests: prometheus::register(
				prometheus::CounterVec::new(
					prometheus::Opts::new(
						"parachain_runtime_api_requests_total",
						"Number of Runtime API requests served.",
					),
					&["success"],
				)?,
				registry,
			)?,
			make_runtime_api_request: prometheus::register(
				prometheus::Histogram::with_opts(
					prometheus::HistogramOpts::new(
						"parachain_runtime_api_make_runtime_api_request",
						"Time spent within `runtime_api::make_runtime_api_request`",
					)
				)?,
				registry,
			)?,
		};
		Ok(Metrics(Some(metrics)))
	}
}
