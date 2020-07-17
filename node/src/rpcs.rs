
#[rpc]
pub trait GuessRpc<Hash> {
    #[rpc(name = "guess_seed")]
    fn guess_seed(&self, Option<Vec<u8>>) -> Result<Hash>;
}
pub struct Guess<C, M> {
    client: Arc<C>,
    _marker: std::marker::PhantomData<M>
}
impl<C, M> Guess<C, M> {
    pub fn new((client: Arc<C>) -> Self {
        Self { client, _marker: Default::default() }
    }
}

impl<C, Block> GuessRpc<<Block as BlockT>::Hash> for Guess<C, Block> 
where 
    Block: BlockT,
    C: Send + Sync + 'static,
    C: ProvideRuntimeApi,
    C: HeaderBackend<Block>,
    C::Api: SumStorageRuntimeApi<Block, <Block as BlockT>::Hash>,{
    fn guess_seed(&self, seed: Option<Vec<u8>>) -> Result<Hash> {
        let api = self.client.runtime_api();
        let runtime_api_result = api.guess(&seed);
        runtime_api_result.map_err(|e| RpcError {
            code: ErrorCode::ServerError(9876),
            message: "Something wrong".into(),
            data: Some(format!("{:?}", e).into()),
        })
    }
}