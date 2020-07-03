use sp_core::{U256, H256};
use sp_runtime::generic::BlockId;
use sp_runtime::traits::Block as BlockT;
use parity_scale_codec::{Encode, Decode};
use sc_consensus_pow::{PowAlgorithm, Error};
use sp_consensus_pow::{ DifficultyApi, Seal as RawSeal};
use sha3::{Sha3_256, Digest};
use rand::{thread_rng, SeedableRng, rngs::SmallRng};
use std::time::Duration;
use std::sync::Arc;
use sp_api::ProvideRuntimeApi;

pub struct Sha3Algorithm<C> {
    client: Arc<C>
}
impl<C> Sha3Algorithm<C> {
    pub fn new(client: Arc<C>) -> Self {
        Self { client }
    }
}
impl<C> Clone for Sha3Algorithm<C> {
    fn clone(&self) -> Self {
        Self::new(self.client.clone())
    } 
}

fn hash_meets_difficulty(hash: &H256, difficulty: U256) -> bool {
    let num_hash = U256::from(&hash[..]);
    let (_, overflowed) = num_hash.overflowing_mul(difficulty);
    !overflowed
}
#[derive(Clone, PartialEq, Eq, Encode, Decode, Debug)]
pub struct Seal {
    pub difficulty: U256,
    pub work: H256,
    pub nonce: H256,
}
#[derive(Clone, PartialEq, Eq, Encode, Decode, Debug)]
pub struct Compute {
    pub difficulty: U256,
    pub pre_hash: H256,
    pub nonce: H256,
}
impl Compute {
    pub fn compute(self) -> Seal {
        let work = H256::from_slice(Sha3_256::digest(&self.encode()[..]).as_slice());
        Seal {
            nonce: self.nonce,
            difficulty: self.difficulty,
            work: H256::from(work),
        }
    }
}
impl<B: BlockT<Hash=H256>, C> PowAlgorithm<B> for Sha3Algorithm<C> where C: ProvideRuntimeApi<B>, C::Api: DifficultyApi<B, U256> {
    type Difficulty = U256;
    fn difficulty(&self, _parent: B::Hash) -> Result<Self::Difficulty, Error<B>> {
        let parent_id = BlockId::<B>::hash(_parent);
        self.client.runtime_api().difficulty(&parent_id).map_err(|e| sc_consensus_pow::Error::Environment(
            format!("Fetching difficulty from runtime failed: {:?}", e)
        ))
    }
    fn verify(&self, _parent: &BlockId<B>, pre_hash: &H256, seal: &RawSeal, difficulty: Self::Difficulty) -> Result<bool, Error<B>> {
        let seal = match Seal::decode(&mut &seal[..]) {
            Ok(seal) => seal,
            Err(_) => return Ok(false)
        };
        if !hash_meets_difficulty(&seal.work, difficulty) {
            return Ok(false)
        }
        let compute = Compute {
            difficulty,
            pre_hash: *pre_hash,
            nonce: seal.nonce,
        };
        if compute.compute() != seal {
            return Ok(false)
        }
        Ok(true)
     }
     fn mine(
         &self,
         _parent: &BlockId<B>,
         pre_hash: &H256,
         difficulty: Self::Difficulty,
         round: u32
     ) -> Result<Option<RawSeal>, Error<B>> {
         let mut rng = SmallRng::from_rng(&mut thread_rng()).map_err(|e| Error::Environment(format!("initialize rng failed for mining: {:?}", e)))?;
         for _ in 0..round {
             let nonce = H256::random_using(&mut rng);
             let compute = Compute {
                 difficulty,
                 pre_hash: *pre_hash,
                 nonce
             };
             let seal = compute.compute();
             if hash_meets_difficulty(&seal.work, difficulty) {
                 return Ok(Some(seal.encode()))
             }
         }
         Ok(None )
    }
}