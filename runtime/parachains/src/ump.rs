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

use crate::{
	configuration::{self, HostConfiguration},
	initializer,
};
use sp_std::{fmt, prelude::*};
use sp_std::collections::{btree_map::BTreeMap, vec_deque::VecDeque};
use frame_support::{decl_module, decl_storage, StorageMap, StorageValue, weights::Weight, traits::Get};
use primitives::v1::{Id as ParaId, UpwardMessage};

/// All upward messages coming from parachains will be funneled into an implementation of this trait.
///
/// The message is opaque from the perspective of UMP. The message size can range from 0 to
/// `config.max_upward_message_size`.
///
/// It's up to the implementation of this trait to decide what to do with a message as long as it
/// returns the amount of weight consumed in the process of handling. Ignoring a message is a valid
/// strategy.
///
/// There are no guarantees on how much time it takes for the message sent by a candidate to end up
/// in the sink after the candidate was enacted. That typically depends on the UMP traffic, the sizes
/// of upward messages and the configuration of UMP.
///
/// It is possible that by the time the message is sank the origin parachain was offboarded. It is
/// up to the implementer to check that if it cares.
pub trait UmpSink {
	/// Process an incoming upward message and return the amount of weight it consumed.
	///
	/// See the trait docs for more details.
	fn process_upward_message(origin: ParaId, msg: Vec<u8>) -> Weight;
}

/// An implementation of a sink that just swallows the message without consuming any weight.
impl UmpSink for () {
	fn process_upward_message(_: ParaId, _: Vec<u8>) -> Weight {
		0
	}
}

/// An error returned by [`check_upward_messages`] that indicates a violation of one of acceptance
/// criteria rules.
pub enum AcceptanceCheckErr {
	MoreMessagesThanPermitted {
		sent: u32,
		permitted: u32,
	},
	MessageSize {
		idx: u32,
		msg_size: u32,
		max_size: u32,
	},
	CapacityExceeded {
		count: u32,
		limit: u32,
	},
	TotalSizeExceeded {
		total_size: u32,
		limit: u32,
	},
}

impl fmt::Debug for AcceptanceCheckErr {
	fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
		match *self {
			AcceptanceCheckErr::MoreMessagesThanPermitted { sent, permitted } => write!(
				fmt,
				"more upward messages than permitted by config ({} > {})",
				sent, permitted,
			),
			AcceptanceCheckErr::MessageSize {
				idx,
				msg_size,
				max_size,
			} => write!(
				fmt,
				"upward message idx {} larger than permitted by config ({} > {})",
				idx, msg_size, max_size,
			),
			AcceptanceCheckErr::CapacityExceeded { count, limit } => write!(
				fmt,
				"the ump queue would have more items than permitted by config ({} > {})",
				count, limit,
			),
			AcceptanceCheckErr::TotalSizeExceeded { total_size, limit } => write!(
				fmt,
				"the ump queue would have grown past the max size permitted by config ({} > {})",
				total_size, limit,
			),
		}
	}
}

pub trait Config: frame_system::Config + configuration::Config {
	/// A place where all received upward messages are funneled.
	type UmpSink: UmpSink;
}

decl_storage! {
	trait Store for Module<T: Config> as Ump {
		/// Paras that are to be cleaned up at the end of the session.
		/// The entries are sorted ascending by the para id.
		OutgoingParas: Vec<ParaId>;

		/// The messages waiting to be handled by the relay-chain originating from a certain parachain.
		///
		/// Note that some upward messages might have been already processed by the inclusion logic. E.g.
		/// channel management messages.
		///
		/// The messages are processed in FIFO order.
		RelayDispatchQueues: map hasher(twox_64_concat) ParaId => VecDeque<UpwardMessage>;
		/// Size of the dispatch queues. Caches sizes of the queues in `RelayDispatchQueue`.
		///
		/// First item in the tuple is the count of messages and second
		/// is the total length (in bytes) of the message payloads.
		///
		/// Note that this is an auxilary mapping: it's possible to tell the byte size and the number of
		/// messages only looking at `RelayDispatchQueues`. This mapping is separate to avoid the cost of
		/// loading the whole message queue if only the total size and count are required.
		///
		/// Invariant:
		/// - The set of keys should exactly match the set of keys of `RelayDispatchQueues`.
		RelayDispatchQueueSize: map hasher(twox_64_concat) ParaId => (u32, u32);
		/// The ordered list of `ParaId`s that have a `RelayDispatchQueue` entry.
		///
		/// Invariant:
		/// - The set of items from this vector should be exactly the set of the keys in
		///   `RelayDispatchQueues` and `RelayDispatchQueueSize`.
		NeedsDispatch: Vec<ParaId>;
		/// This is the para that gets will get dispatched first during the next upward dispatchable queue
		/// execution round.
		///
		/// Invariant:
		/// - If `Some(para)`, then `para` must be present in `NeedsDispatch`.
		NextDispatchRoundStartWith: Option<ParaId>;
	}
}

decl_module! {
	/// The UMP module.
	pub struct Module<T: Config> for enum Call where origin: <T as frame_system::Config>::Origin {
	}
}

/// Routines related to the upward message passing.
impl<T: Config> Module<T> {
	/// Block initialization logic, called by initializer.
	pub(crate) fn initializer_initialize(_now: T::BlockNumber) -> Weight {
		0
	}

	/// Block finalization logic, called by initializer.
	pub(crate) fn initializer_finalize() {}

	/// Called by the initializer to note that a new session has started.
	pub(crate) fn initializer_on_new_session(
		_notification: &initializer::SessionChangeNotification<T::BlockNumber>,
	) {
		Self::perform_outgoing_para_cleanup();
	}

	/// Iterate over all paras that were registered for offboarding and remove all the data
	/// associated with them.
	fn perform_outgoing_para_cleanup() {
		let outgoing = OutgoingParas::take();
		for outgoing_para in outgoing {
			Self::clean_ump_after_outgoing(outgoing_para);
		}
	}

	/// Schedule a para to be cleaned up at the start of the next session.
	pub(crate) fn schedule_para_cleanup(id: ParaId) {
		OutgoingParas::mutate(|v| {
			if let Err(i) = v.binary_search(&id) {
				v.insert(i, id);
			}
		});
	}

	fn clean_ump_after_outgoing(outgoing_para: ParaId) {
		<Self as Store>::RelayDispatchQueueSize::remove(&outgoing_para);
		<Self as Store>::RelayDispatchQueues::remove(&outgoing_para);

		// Remove the outgoing para from the `NeedsDispatch` list and from
		// `NextDispatchRoundStartWith`.
		//
		// That's needed for maintaining invariant that `NextDispatchRoundStartWith` points to an
		// existing item in `NeedsDispatch`.
		<Self as Store>::NeedsDispatch::mutate(|v| {
			if let Ok(i) = v.binary_search(&outgoing_para) {
				v.remove(i);
			}
		});
		<Self as Store>::NextDispatchRoundStartWith::mutate(|v| {
			*v = v.filter(|p| *p == outgoing_para)
		});
	}

	/// Check that all the upward messages sent by a candidate pass the acceptance criteria. Returns
	/// false, if any of the messages doesn't pass.
	pub(crate) fn check_upward_messages(
		config: &HostConfiguration<T::BlockNumber>,
		para: ParaId,
		upward_messages: &[UpwardMessage],
	) -> Result<(), AcceptanceCheckErr> {
		if upward_messages.len() as u32 > config.max_upward_message_num_per_candidate {
			return Err(AcceptanceCheckErr::MoreMessagesThanPermitted {
				sent: upward_messages.len() as u32,
				permitted: config.max_upward_message_num_per_candidate,
			});
		}

		let (mut para_queue_count, mut para_queue_size) =
			<Self as Store>::RelayDispatchQueueSize::get(&para);

		for (idx, msg) in upward_messages.into_iter().enumerate() {
			let msg_size = msg.len() as u32;
			if msg_size > config.max_upward_message_size {
				return Err(AcceptanceCheckErr::MessageSize {
					idx: idx as u32,
					msg_size,
					max_size: config.max_upward_message_size,
				});
			}
			para_queue_count += 1;
			para_queue_size += msg_size;
		}

		// make sure that the queue is not overfilled.
		// we do it here only once since returning false invalidates the whole relay-chain block.
		if para_queue_count > config.max_upward_queue_count {
			return Err(AcceptanceCheckErr::CapacityExceeded {
				count: para_queue_count,
				limit: config.max_upward_queue_count,
			});
		}
		if para_queue_size > config.max_upward_queue_size {
			return Err(AcceptanceCheckErr::TotalSizeExceeded {
				total_size: para_queue_size,
				limit: config.max_upward_queue_size,
			});
		}

		Ok(())
	}

	/// Enacts all the upward messages sent by a candidate.
	pub(crate) fn enact_upward_messages(
		para: ParaId,
		upward_messages: Vec<UpwardMessage>,
	) -> Weight {
		let mut weight = 0;

		if !upward_messages.is_empty() {
			let (extra_cnt, extra_size) = upward_messages
				.iter()
				.fold((0, 0), |(cnt, size), d| (cnt + 1, size + d.len() as u32));

			<Self as Store>::RelayDispatchQueues::mutate(&para, |v| {
				v.extend(upward_messages.into_iter())
			});

			<Self as Store>::RelayDispatchQueueSize::mutate(&para, |(ref mut cnt, ref mut size)| {
				*cnt += extra_cnt;
				*size += extra_size;
			});

			<Self as Store>::NeedsDispatch::mutate(|v| {
				if let Err(i) = v.binary_search(&para) {
					v.insert(i, para);
				}
			});

			weight += T::DbWeight::get().reads_writes(3, 3);
		}

		weight
	}

	/// Devote some time into dispatching pending upward messages.
	pub(crate) fn process_pending_upward_messages() {
		let mut used_weight_so_far = 0;

		let config = <configuration::Module<T>>::config();
		let mut cursor = NeedsDispatchCursor::new::<T>();
		let mut queue_cache = QueueCache::new();

		while let Some(dispatchee) = cursor.peek() {
			if used_weight_so_far >= config.preferred_dispatchable_upward_messages_step_weight {
				// Then check whether we've reached or overshoot the
				// preferred weight for the dispatching stage.
				//
				// if so - bail.
				break;
			}

			// dequeue the next message from the queue of the dispatchee
			let (upward_message, became_empty) = queue_cache.dequeue::<T>(dispatchee);
			if let Some(upward_message) = upward_message {
				used_weight_so_far +=
					T::UmpSink::process_upward_message(dispatchee, upward_message);
			}

			if became_empty {
				// the queue is empty now - this para doesn't need attention anymore.
				cursor.remove();
			} else {
				cursor.advance();
			}
		}

		cursor.flush::<T>();
		queue_cache.flush::<T>();
	}
}

/// To avoid constant fetching, deserializing and serialization the queues are cached.
///
/// After an item dequeued from a queue for the first time, the queue is stored in this struct rather
/// than being serialized and persisted.
///
/// This implementation works best when:
///
/// 1. when the queues are shallow
/// 2. the dispatcher makes more than one cycle
///
/// if the queues are deep and there are many we would load and keep the queues for a long time,
/// thus increasing the peak memory consumption of the wasm runtime. Under such conditions persisting
/// queues might play better since it's unlikely that they are going to be requested once more.
///
/// On the other hand, the situation when deep queues exist and it takes more than one dipsatcher
/// cycle to traverse the queues is already sub-optimal and better be avoided.
///
/// This struct is not supposed to be dropped but rather to be consumed by [`flush`].
struct QueueCache(BTreeMap<ParaId, QueueCacheEntry>);

struct QueueCacheEntry {
	queue: VecDeque<UpwardMessage>,
	count: u32,
	total_size: u32,
}

impl QueueCache {
	fn new() -> Self {
		Self(BTreeMap::new())
	}

	/// Dequeues one item from the upward message queue of the given para.
	///
	/// Returns `(upward_message, became_empty)`, where
	///
	/// - `upward_message` a dequeued message or `None` if the queue _was_ empty.
	/// - `became_empty` is true if the queue _became_ empty.
	fn dequeue<T: Config>(&mut self, para: ParaId) -> (Option<UpwardMessage>, bool) {
		let cache_entry = self.0.entry(para).or_insert_with(|| {
			let queue = <Module<T> as Store>::RelayDispatchQueues::get(&para);
			let (count, total_size) = <Module<T> as Store>::RelayDispatchQueueSize::get(&para);
			QueueCacheEntry {
				queue,
				count,
				total_size,
			}
		});
		let upward_message = cache_entry.queue.pop_front();
		if let Some(ref msg) = upward_message {
			cache_entry.count -= 1;
			cache_entry.total_size -= msg.len() as u32;
		}

		let became_empty = cache_entry.queue.is_empty();
		(upward_message, became_empty)
	}

	/// Flushes the updated queues into the storage.
	fn flush<T: Config>(self) {
		// NOTE we use an explicit method here instead of Drop impl because it has unwanted semantics
		// within runtime. It is dangerous to use because of double-panics and flushing on a panic
		// is not necessary as well.
		for (
			para,
			QueueCacheEntry {
				queue,
				count,
				total_size,
			},
		) in self.0
		{
			if queue.is_empty() {
				// remove the entries altogether.
				<Module<T> as Store>::RelayDispatchQueues::remove(&para);
				<Module<T> as Store>::RelayDispatchQueueSize::remove(&para);
			} else {
				<Module<T> as Store>::RelayDispatchQueues::insert(&para, queue);
				<Module<T> as Store>::RelayDispatchQueueSize::insert(&para, (count, total_size));
			}
		}
	}
}

/// A cursor that iterates over all entries in `NeedsDispatch`.
///
/// This cursor will start with the para indicated by `NextDispatchRoundStartWith` storage entry.
/// This cursor is cyclic meaning that after reaching the end it will jump to the beginning. Unlike
/// an iterator, this cursor allows removing items during the iteration.
///
/// Each iteration cycle *must be* concluded with a call to either `advance` or `remove`.
///
/// This struct is not supposed to be dropped but rather to be consumed by [`flush`].
#[derive(Debug)]
struct NeedsDispatchCursor {
	needs_dispatch: Vec<ParaId>,
	cur_idx: usize,
}

impl NeedsDispatchCursor {
	fn new<T: Config>() -> Self {
		let needs_dispatch: Vec<ParaId> = <Module<T> as Store>::NeedsDispatch::get();
		let start_with = <Module<T> as Store>::NextDispatchRoundStartWith::get();

		let start_with_idx = match start_with {
			Some(para) => match needs_dispatch.binary_search(&para) {
				Ok(found_idx) => found_idx,
				Err(_supposed_idx) => {
					// well that's weird because we maintain an invariant that
					// `NextDispatchRoundStartWith` must point into one of the items in
					// `NeedsDispatch`.
					//
					// let's select 0 as the starting index as a safe bet.
					debug_assert!(false);
					0
				}
			},
			None => 0,
		};

		Self {
			needs_dispatch,
			cur_idx: start_with_idx,
		}
	}

	/// Returns the item the cursor points to.
	fn peek(&self) -> Option<ParaId> {
		self.needs_dispatch.get(self.cur_idx).cloned()
	}

	/// Moves the cursor to the next item.
	fn advance(&mut self) {
		if self.needs_dispatch.is_empty() {
			return;
		}
		self.cur_idx = (self.cur_idx + 1) % self.needs_dispatch.len();
	}

	/// Removes the item under the cursor.
	fn remove(&mut self) {
		if self.needs_dispatch.is_empty() {
			return;
		}
		let _ = self.needs_dispatch.remove(self.cur_idx);

		// we might've removed the last element and that doesn't necessarily mean that `needs_dispatch`
		// became empty. Reposition the cursor in this case to the beginning.
		if self.needs_dispatch.get(self.cur_idx).is_none() {
			self.cur_idx = 0;
		}
	}

	/// Flushes the dispatcher state into the persistent storage.
	fn flush<T: Config>(self) {
		let next_one = self.peek();
		<Module<T> as Store>::NextDispatchRoundStartWith::set(next_one);
		<Module<T> as Store>::NeedsDispatch::put(self.needs_dispatch);
	}
}