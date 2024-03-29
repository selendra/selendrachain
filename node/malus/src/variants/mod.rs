// Copyright 2021 SmallWorld Selendra (Kh).
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

//! Collection of behavior variants.

mod back_garbage_candidate;
mod dispute_valid_candidates;
mod suggest_garbage_candidate;

pub(crate) use self::{
	back_garbage_candidate::BackGarbageCandidate, dispute_valid_candidates::DisputeValidCandidates,
	suggest_garbage_candidate::SuggestGarbageCandidate,
};
