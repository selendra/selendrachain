// Copyright 2017-2021 SmallWorld Selendra (Kh).
// This file is part of Selendra.

// Selendra is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Selendra is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Selendra.  If not, see <http://www.gnu.org/licenses/>.

#[derive(thiserror::Error, Debug)]
pub enum Error {
	#[error(transparent)]
	SelendraService(#[from] service::Error),

	#[error(transparent)]
	SubstrateCli(#[from] sc_cli::Error),

	#[error(transparent)]
	SubstrateService(#[from] sc_service::Error),

	#[error(transparent)]
	SubstrateTracing(#[from] sc_tracing::logging::Error),

	#[error(transparent)]
	PerfCheck(#[from] selendra_performance_test::PerfCheckError),

	#[cfg(not(feature = "pyroscope"))]
	#[error("Binary was not compiled with `--feature=pyroscope`")]
	PyroscopeNotCompiledIn,

	#[cfg(feature = "pyroscope")]
	#[error("Failed to connect to pyroscope agent")]
	PyroscopeError(#[from] pyro::error::PyroscopeError),

	#[error("Failed to resolve provided URL")]
	AddressResolutionFailure(#[from] std::io::Error),

	#[error("URL did not resolve to anything")]
	AddressResolutionMissing,

	#[error("Other: {0}")]
	Other(String),
}
