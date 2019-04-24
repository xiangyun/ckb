use crate::synchronizer::{BlockStatus, Synchronizer};
use crate::types::HeaderView;
use crate::MAX_HEADERS_LEN;
use ckb_core::extras::EpochExt;
use ckb_core::{header::Header, BlockNumber};
use ckb_network::{CKBProtocolContext, PeerIndex};
use ckb_protocol::{cast, FlatbuffersVectorIterator, Headers};
use ckb_store::ChainStore;
use ckb_traits::BlockMedianTimeContext;
use ckb_verification::{Error as VerifyError, HeaderResolver, HeaderVerifier, Verifier};
use failure::Error as FailureError;
use log::{self, debug, log_enabled, warn};
use numext_fixed_uint::U256;
use std::convert::TryInto;
use std::sync::Arc;

pub struct HeadersProcess<'a, CS: ChainStore + 'a> {
    message: &'a Headers<'a>,
    synchronizer: &'a Synchronizer<CS>,
    peer: PeerIndex,
    nc: &'a CKBProtocolContext,
}

pub struct VerifierResolver<'a, CS: ChainStore + 'a> {
    synchronizer: &'a Synchronizer<CS>,
    header: &'a Header,
    parent: Option<&'a Header>,
}

impl<'a, CS: ChainStore + 'a> VerifierResolver<'a, CS> {
    pub fn new(
        parent: Option<&'a Header>,
        header: &'a Header,
        synchronizer: &'a Synchronizer<CS>,
    ) -> Self {
        VerifierResolver {
            parent,
            header,
            synchronizer,
        }
    }
}

impl<'a, CS: ChainStore> ::std::clone::Clone for VerifierResolver<'a, CS> {
    fn clone(&self) -> Self {
        VerifierResolver {
            parent: self.parent,
            header: self.header,
            synchronizer: self.synchronizer,
        }
    }
}

impl<'a, CS: ChainStore + 'a> BlockMedianTimeContext for VerifierResolver<'a, CS> {
    fn median_block_count(&self) -> u64 {
        self.synchronizer
            .shared
            .consensus()
            .median_time_block_count() as u64
    }

    fn timestamp(&self, _n: BlockNumber) -> Option<u64> {
        None
    }

    fn ancestor_timestamps(&self, block_number: BlockNumber) -> Vec<u64> {
        if Some(block_number) != self.parent.and_then(|p| Some(p.number())) {
            return Vec::new();
        }
        let parent = self.parent.expect("parent");
        let count = std::cmp::min(self.median_block_count(), block_number + 1);
        let mut block_hash = parent.hash().to_owned();
        let mut timestamps: Vec<u64> = Vec::with_capacity(count as usize);
        for _ in 0..count {
            let header = match self.synchronizer.shared.get_header(&block_hash) {
                Some(h) => h,
                None => break,
            };
            timestamps.push(header.timestamp());
            block_hash = header.parent_hash().to_owned();
        }
        timestamps
    }
}

impl<'a, CS: ChainStore> HeaderResolver for VerifierResolver<'a, CS> {
    fn header(&self) -> &Header {
        self.header
    }

    fn parent(&self) -> Option<&Header> {
        self.parent
    }

    fn epoch(&self) -> Option<&EpochExt> {
        unimplemented!();
    }

    // fn next_epoch_ext(&self) -> Option<Epoch> {
    //     self.parent().and_then(|parent| {
    //         let start = last_epoch.start();
    //         let last_epoch_length = last_epoch.length();

    //         if !self.is_epoch_end(last_epoch, header.number()) {
    //             return None;
    //         }

    //         let end_hash = header.hash();
    //         let target_recip = self.consensus.orphan_rate_target_recip();
    //         let epoch_duration = self.consensus.epoch_duration();

    //         if let Some(start_header) = self.get_ancestor(&end_hash, start) {
    //             let start_total_uncles_count = self
    //                 .block_ext(&start_header.hash())
    //                 .expect("block_ext exist")
    //                 .total_uncles_count;

    //             let last_total_uncles_count = self
    //                 .block_ext(&last_hash)
    //                 .expect("block_ext exist")
    //                 .total_uncles_count;

    //             let last_duration = header
    //                 .header()
    //                 .timestamp()
    //                 .saturating_sub(start_header.header().timestamp());
    //             if last_duration == 0 {
    //                 return None;
    //             }

    //             let numerator =
    //                 (last_uncles_count + last_epoch_length) * epoch_duration * last_epoch_length;
    //             let denominator = (target_recip + 1) * last_uncles_count * last_duration;
    //             let raw_next_epoch_length = numerator / denominator;
    //             let next_epoch_length = self.fix_epoch_length(raw_next_epoch_length);

    //             let raw_difficulty =
    //                 last_difficulty * U256::from(last_uncles_count) * U256::from(target_recip)
    //                     / U256::from(last_epoch_length);

    //             let difficulty = self.fix_epoch_difficulty(last_difficulty, raw_difficulty);

    //             let block_reward =
    //                 Capacity::shannons(self.consensus.epoch_reward().as_u64() / next_epoch_length);
    //             let remainder_reward =
    //                 Capacity::shannons(self.consensus.epoch_reward().as_u64() / next_epoch_length);

    //             let epoch = EpochExt::new(
    //                 last_epoch.number() + 1, // number
    //                 block_reward,
    //                 remainder_reward,        // remainder_reward
    //                 header.number() + 1,     // start
    //                 next_epoch_length,       // length
    //                 difficulty               // difficulty,
    //             );
    //             Some(epoch)
    //         } else {
    //             None
    //         }
    //     })
    // }

    // fn calculate_difficulty(&self) -> Option<U256> {
    //     self.parent().and_then(|parent| {
    //         let parent_hash = parent.hash();
    //         let parent_number = parent.number();
    //         let last_difficulty = parent.difficulty();

    //         let interval = self
    //             .synchronizer
    //             .consensus()
    //             .difficulty_adjustment_interval();

    //         if self.header().number() % interval != 0 {
    //             return Some(last_difficulty.clone());
    //         }

    //         let start = parent_number.saturating_sub(interval);

    //         if let Some(start_header) = self.synchronizer.get_ancestor(&parent_hash, start) {
    //             let start_total_uncles_count = self
    //                 .synchronizer
    //                 .get_header_view(&start_header.hash())
    //                 .expect("start header_view exist")
    //                 .total_uncles_count();

    //             let last_total_uncles_count = self
    //                 .synchronizer
    //                 .get_header_view(&parent_hash)
    //                 .expect("last header_view exist")
    //                 .total_uncles_count();

    //             let difficulty = last_difficulty
    //                 * U256::from(last_total_uncles_count - start_total_uncles_count)
    //                 * U256::from((1.0 / self.synchronizer.consensus().orphan_rate_target()) as u64)
    //                 / U256::from(interval);

    //             let min_difficulty = self.synchronizer.consensus().min_difficulty();
    //             let max_difficulty = last_difficulty * 2u32;
    //             if difficulty > max_difficulty {
    //                 return Some(max_difficulty);
    //             }

    //             if difficulty.lt(min_difficulty) {
    //                 return Some(min_difficulty.clone());
    //             }
    //             return Some(difficulty);
    //         }
    //         None
    //     })
    // }
}

impl<'a, CS> HeadersProcess<'a, CS>
where
    CS: ChainStore + 'a,
{
    pub fn new(
        message: &'a Headers,
        synchronizer: &'a Synchronizer<CS>,
        peer: PeerIndex,
        nc: &'a CKBProtocolContext,
    ) -> Self {
        HeadersProcess {
            message,
            nc,
            synchronizer,
            peer,
        }
    }

    fn is_continuous(&self, headers: &[Header]) -> bool {
        for window in headers.windows(2) {
            if let [parent, header] = &window {
                if header.parent_hash() != &parent.hash() {
                    debug!(
                        target: "sync",
                        "header.parent_hash {:?} parent.hash {:?}",
                        header.parent_hash(),
                        parent.hash()
                    );
                    return false;
                }
            }
        }
        true
    }

    fn received_new_header(&self, headers: &[Header]) -> bool {
        let last = headers.last().expect("empty checked");
        self.synchronizer.get_block_status(&last.hash()) == BlockStatus::UNKNOWN
    }

    pub fn accept_first(&self, first: &Header) -> ValidationResult {
        let parent = self.synchronizer.shared.get_header(&first.parent_hash());
        let resolver = VerifierResolver::new(parent.as_ref(), &first, &self.synchronizer);
        let verifier = HeaderVerifier::new(
            resolver.clone(),
            Arc::clone(&self.synchronizer.shared.consensus().pow_engine()),
        );
        let acceptor =
            HeaderAcceptor::new(first, self.peer, &self.synchronizer, resolver, verifier);
        acceptor.accept()
    }

    pub fn execute(self) -> Result<(), FailureError> {
        debug!(target: "sync", "HeadersProcess begin");

        let headers = cast!(self.message.headers())?;

        if headers.len() > MAX_HEADERS_LEN {
            self.synchronizer.peers.misbehavior(self.peer, 20);
            warn!(target: "sync", "HeadersProcess is_oversize");
            return Ok(());
        }

        let best_known_header = self.synchronizer.shared.best_known_header();
        if headers.len() == 0 {
            // Update peer's best known header
            self.synchronizer
                .peers
                .best_known_headers
                .write()
                .insert(self.peer, best_known_header);
            // Reset headers sync timeout
            self.synchronizer
                .peers
                .state
                .write()
                .get_mut(&self.peer)
                .expect("Peer must exists")
                .headers_sync_timeout = None;
            debug!(target: "sync", "HeadersProcess is_empty (synchronized)");
            return Ok(());
        }

        let headers = FlatbuffersVectorIterator::new(headers)
            .map(TryInto::try_into)
            .collect::<Result<Vec<Header>, FailureError>>()?;

        if !self.is_continuous(&headers) {
            self.synchronizer.peers.misbehavior(self.peer, 20);
            debug!(target: "sync", "HeadersProcess is not continuous");
            return Ok(());
        }

        let result = self.accept_first(&headers[0]);
        if !result.is_valid() {
            if result.misbehavior > 0 {
                self.synchronizer
                    .peers
                    .misbehavior(self.peer, result.misbehavior);
            }
            debug!(target: "sync", "\n\nHeadersProcess accept_first is_valid {:?} headers = {:?}\n\n", result, headers[0]);
            return Ok(());
        }

        for window in headers.windows(2) {
            if let [parent, header] = &window {
                let resolver = VerifierResolver::new(Some(&parent), &header, &self.synchronizer);
                let verifier = HeaderVerifier::new(
                    resolver.clone(),
                    Arc::clone(&self.synchronizer.shared.consensus().pow_engine()),
                );
                let acceptor =
                    HeaderAcceptor::new(&header, self.peer, &self.synchronizer, resolver, verifier);
                let result = acceptor.accept();

                if !result.is_valid() {
                    if result.misbehavior > 0 {
                        self.synchronizer
                            .peers
                            .misbehavior(self.peer, result.misbehavior);
                    }
                    debug!(target: "sync", "HeadersProcess accept is invalid {:?}", result);
                    return Ok(());
                }
            }
        }

        if log_enabled!(target: "sync", log::Level::Debug) {
            let chain_state = self.synchronizer.shared.chain_state().lock();
            let peer_state = self.synchronizer.peers.best_known_header(self.peer);
            debug!(
                target: "sync",
                concat!(
                    "\n\nchain total_difficulty = {}; number={}\n",
                    "number={}; best_known_header = {:x}; total_difficulty = {};\n",
                    "peers={} number={:?}; best_known_header = {:?}; total_difficulty = {:?}\n",
                ),
                chain_state.total_difficulty(),
                chain_state.tip_number(),
                best_known_header.number(),
                best_known_header.hash(),
                best_known_header.total_difficulty(),
                self.peer,
                peer_state.as_ref().map(HeaderView::number),
                peer_state.as_ref().map(|state| format!("{:x}", state.hash())),
                peer_state.as_ref().map(|state| format!("{}", state.total_difficulty())),
            );
        }

        if self.received_new_header(&headers) {
            // update peer last_block_announcement
        }

        // TODO: optimize: if last is an ancestor of BestKnownHeader, continue from there instead.
        if headers.len() == MAX_HEADERS_LEN {
            let start = headers.last().expect("empty checked");
            self.synchronizer
                .shared
                .send_getheaders_to_peer(self.nc, self.peer, start);
        }

        // If we're in IBD, we want outbound peers that will serve us a useful
        // chain. Disconnect peers that are on chains with insufficient work.
        let is_outbound = self
            .nc
            .get_peer(self.peer)
            .map(|peer| peer.is_outbound())
            .unwrap_or(false);
        let is_protected = self
            .synchronizer
            .peers
            .state
            .read()
            .get(&self.peer)
            .map(|state| state.chain_sync.protect)
            .unwrap_or(false);
        if self.synchronizer.shared.is_initial_block_download()
            && headers.len() != MAX_HEADERS_LEN
            && (is_outbound && !is_protected)
        {
            debug!(target: "sync", "Disconnect peer({}) is unprotected outbound", self.peer);
            self.nc.disconnect(self.peer);
        }

        Ok(())
    }
}

#[derive(Clone)]
pub struct HeaderAcceptor<'a, V: Verifier, CS: ChainStore + 'a> {
    header: &'a Header,
    synchronizer: &'a Synchronizer<CS>,
    peer: PeerIndex,
    resolver: V::Target,
    verifier: V,
}

impl<'a, V, CS> HeaderAcceptor<'a, V, CS>
where
    V: Verifier,
    CS: ChainStore + 'a,
{
    pub fn new(
        header: &'a Header,
        peer: PeerIndex,
        synchronizer: &'a Synchronizer<CS>,
        resolver: V::Target,
        verifier: V,
    ) -> Self {
        HeaderAcceptor {
            header,
            peer,
            resolver,
            verifier,
            synchronizer,
        }
    }

    //FIXME: status flag
    pub fn duplicate_check(&self, _state: &mut ValidationResult) -> Result<(), ()> {
        // let status = self.synchronizer.get_block_status(&self.header.hash());
        // if status != BlockStatus::UNKNOWN {
        //     if (status & BlockStatus::FAILED_MASK) == status {
        //         state.invalid(Some(ValidationError::FailedMask));
        //     }
        //     if (status & BlockStatus::FAILED_MASK) == status {}
        //     return Err(());
        // }
        Ok(())
    }

    pub fn prev_block_check(&self, state: &mut ValidationResult) -> Result<(), ()> {
        let status = self
            .synchronizer
            .get_block_status(&self.header.parent_hash());

        if (status & BlockStatus::FAILED_MASK) == status {
            state.dos(Some(ValidationError::InvalidParent), 100);
            return Err(());
        }
        Ok(())
    }

    pub fn non_contextual_check(&self, state: &mut ValidationResult) -> Result<(), ()> {
        self.verifier.verify(&self.resolver).map_err(|error| match error {
            VerifyError::Pow(e) => {
                debug!(target: "sync", "HeadersProcess accept {:?} pow", self.header.number());
                state.dos(Some(ValidationError::Verify(VerifyError::Pow(e))), 100);
            }
            VerifyError::Difficulty(e) => {
                debug!(target: "sync", "HeadersProcess accept {:?} difficulty", self.header.number());
                state.dos(
                    Some(ValidationError::Verify(VerifyError::Difficulty(e))),
                    50,
                );
            }
            error => {
                debug!(target: "sync", "HeadersProcess accept {:?} {:?}", self.header.number(), error);
                state.invalid(Some(ValidationError::Verify(error)));
            }
        })
    }

    pub fn version_check(&self, state: &mut ValidationResult) -> Result<(), ()> {
        if self.header.version() != 0 {
            state.invalid(Some(ValidationError::Version));
            Err(())
        } else {
            Ok(())
        }
    }

    pub fn accept(&self) -> ValidationResult {
        let mut result = ValidationResult::default();
        if self.duplicate_check(&mut result).is_err() {
            debug!(target: "sync", "HeadersProcess accept {:?} duplicate", self.header.number());
            return result;
        }

        if self.prev_block_check(&mut result).is_err() {
            debug!(target: "sync", "HeadersProcess accept {:?} prev_block", self.header.number());
            self.synchronizer
                .insert_block_status(self.header.hash(), BlockStatus::FAILED_MASK);
            return result;
        }

        if self.non_contextual_check(&mut result).is_err() {
            debug!(target: "sync", "HeadersProcess accept {:?} non_contextual", self.header.number());
            self.synchronizer
                .insert_block_status(self.header.hash(), BlockStatus::FAILED_MASK);
            return result;
        }

        if self.version_check(&mut result).is_err() {
            debug!(target: "sync", "HeadersProcess accept {:?} version", self.header.number());
            self.synchronizer
                .insert_block_status(self.header.hash(), BlockStatus::FAILED_MASK);
            return result;
        }

        self.synchronizer
            .insert_header_view(&self.header, self.peer);
        self.synchronizer
            .insert_block_status(self.header.hash(), BlockStatus::VALID_MASK);
        result
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ValidationState {
    VALID,
    INVALID,
}

impl Default for ValidationState {
    fn default() -> Self {
        ValidationState::VALID
    }
}

#[derive(Debug)]
pub enum ValidationError {
    Verify(VerifyError),
    Version,
    InvalidParent,
}

#[derive(Debug, Default)]
pub struct ValidationResult {
    pub error: Option<ValidationError>,
    pub misbehavior: u32,
    pub state: ValidationState,
}

impl ValidationResult {
    pub fn invalid(&mut self, error: Option<ValidationError>) {
        self.dos(error, 0);
    }

    pub fn dos(&mut self, error: Option<ValidationError>, misbehavior: u32) {
        self.error = error;
        self.misbehavior += misbehavior;
        self.state = ValidationState::INVALID;
    }

    pub fn is_valid(&self) -> bool {
        self.state == ValidationState::VALID
    }
}
