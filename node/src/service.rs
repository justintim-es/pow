//! Service and ServiceFactory implementation. Specialized wrapper over substrate service.

use std::sync::Arc;
use std::time::Duration;
use sc_client_api::ExecutorProvider;
use sc_consensus::LongestChain;
use node_template_runtime::{self, opaque::Block, RuntimeApi};
use sc_service::{error::{Error as ServiceError}, AbstractService, Configuration, ServiceBuilder};
use sp_inherents::InherentDataProviders;
use sc_network::config::DummyFinalityProofRequestBuilder;
use sc_executor::native_executor_instance;
pub use sc_executor::NativeExecutor;
use sp_consensus_aura::sr25519::{AuthorityPair as AuraPair};
use sc_finality_grandpa::{
	FinalityProofProvider as GrandpaFinalityProofProvider, StorageAndProofProvider, SharedVoterState,
};
use sp_api::ProvideRuntimeApi;
use sp_consensus_pow::DifficultyApi;
use sp_core::{U256, H256};
type RpcExtension = jsonrpc_core::IoHandler<sc_rpc::Metadata>;
// pub mod pow;
// Our native executor instance.
native_executor_instance!(
	pub Executor,
	node_template_runtime::api::dispatch,
	node_template_runtime::native_version,
);

/// Starts a `ServiceBuilder` for a full service.
///
/// Use this macro if you don't actually need the full service, but just the builder in order to
/// be able to perform chain operations.
macro_rules! new_full_start {
	($config:expr) => {{
		// use std::sync::Arc;
		type RpcExtension = jsonrpc_core::IoHandler<sc_rpc::Metadata>;
		// use sp_consensus_aura::sr25519::AuthorityPair as AuraPair;
		use jsonrpc_core::IoHandler;
		let mut import_setup = None;
		let inherent_data_providers = sp_inherents::InherentDataProviders::new();

		let builder = sc_service::ServiceBuilder::new_full::<
			node_template_runtime::opaque::Block,
			node_template_runtime::RuntimeApi,
			crate::service::Executor
		>($config)?
			.with_select_chain(|_config, backend| {
				Ok(sc_consensus::LongestChain::new(backend.clone()))
 			})?
			.with_transaction_pool(|builder| {
				let pool_api = sc_transaction_pool::FullChainApi::new(
					builder.client().clone(),
				);
				Ok(sc_transaction_pool::BasicPool::new(
					builder.config().transaction_pool.clone(),
					std::sync::Arc::new(pool_api),
					builder.prometheus_registry(),
				))
			})?
			.with_import_queue(|
				_config,
				client,
				mut select_chain,
				_transaction_pool,
				spawn_task_handle,
				registry,
			| {
				let pow_block_import = sc_consensus_pow::PowBlockImport::new(
					client.clone(),
					client.clone(),
					crate::pow::Sha3Algorithm::new(client.clone()),
					0,
					select_chain,
					inherent_data_providers.clone()
				);
				let import_queue = sc_consensus_pow::import_queue(
					Box::new(pow_block_import.clone()),
					None,
					None,
					crate::pow::Sha3Algorithm::new(client.clone()),
					inherent_data_providers.clone(),
					spawn_task_handle,
					registry
				)?;

				import_setup = Some(pow_block_import);

				Ok(import_queue)
			})?.with_rpc_extensions(|builder| -> Result<RpcExtension, _> {
                let handler = pallet_contracts_rpc::Contracts::new(builder.client().clone());
				let delegate = pallet_contracts_rpc::ContractsApi::to_delegate(handler);
				
				let mut io = jsonrpc_core::IoHandler::default();
				io.extend_with(delegate);
				io.extend_with(rpcs::GuessRpc::to_delegate(rpcs::Guess::new(builder.client().clone())));
				Ok(io)
            })?;

		(builder, import_setup, inherent_data_providers)
	}}
}

/// Builds a new service for a full client.
pub fn new_full(config: Configuration) -> Result<impl AbstractService, ServiceError> {
	let role = config.role.clone();
	// let force_authoring = config.force_authoring;
	// let name = config.network.node_name.clone();
	// let disable_grandpa = config.disable_grandpa;

	let (builder, mut import_setup, inherent_data_providers) = new_full_start!(config);

	let block_import =import_setup.take()
			.expect("Link Half and Block Import are present for Full Services or setup failed before. qed");

	let service = builder
	.with_finality_proof_provider(|client, backend| {
		// GenesisAuthoritySetProvider is implemented for StorageAndProofProvider
		let provider = client as Arc<dyn StorageAndProofProvider<_, _>>;
		Ok(Arc::new(()) as _)
	})?
	.build_full()?;

	let proposer = sc_basic_authorship::ProposerFactory::new(
		service.client(),
		service.transaction_pool(),
		service.prometheus_registry().as_ref(),
	);
	let rounds = 500;

	let client = service.client();
	let select_chain = service.select_chain()
		.ok_or(ServiceError::SelectChainRequired)?;

	let can_author_with =
		sp_consensus::CanAuthorWithNativeVersion::new(client.executor().clone());


	sc_consensus_pow::start_mine(
		Box::new(block_import),
		client.clone(),
		crate::pow::Sha3Algorithm::new(client.clone()),
		proposer,
		None,
		rounds,
		service.network(),
		std::time::Duration::new(2,0),
		Some(select_chain),
		inherent_data_providers,
		can_author_with
	);

	Ok(service)
}

/// Builds a new service for a light client.
pub fn new_light(config: Configuration) -> Result<impl AbstractService, ServiceError> {
	let inherent_data_providers = InherentDataProviders::new();

	ServiceBuilder::new_light::<Block, RuntimeApi, Executor>(config)?
		.with_select_chain(|_config, backend| {
			Ok(LongestChain::new(backend.clone()))
		})?
		.with_transaction_pool(|builder| {
			let fetcher = builder.fetcher()
				.ok_or_else(|| "Trying to start light transaction pool without active fetcher")?;

			let pool_api = sc_transaction_pool::LightChainApi::new(
				builder.client().clone(),
				fetcher.clone(),
			);
			let pool = sc_transaction_pool::BasicPool::with_revalidation_type(
				builder.config().transaction_pool.clone(),
				Arc::new(pool_api),
				builder.prometheus_registry(),
				sc_transaction_pool::RevalidationType::Light,
			);
			Ok(pool)
		})?
		.with_import_queue_and_fprb(|
			_config,
			client,
			backend,
			fetcher,
			_select_chain,
			_tx_pool,
			spawn_task_handle,
			prometheus_registry,
		| {
			// let fetch_checker = fetcher
			// 	.map(|fetcher| fetcher.checker().clone())
			// 	.ok_or_else(|| "Trying to start light import queue without active fetch checker")?;
			// let grandpa_block_import = sc_finality_grandpa::light_block_import(
			// 	client.clone(),
			// 	backend,
			// 	&(client.clone() as Arc<_>),
			// 	Arc::new(fetch_checker),
			// )?;
			// let finality_proof_import = grandpa_block_import.clone();
			// let finality_proof_request_builder =
			// 	finality_proof_import.create_finality_proof_request_builder();

			// let import_queue = sc_consensus_aura::import_queue::<_, _, _, AuraPair, _>(
			// 	sc_consensus_aura::slot_duration(&*client)?,
			// 	grandpa_block_import,
			// 	None,
			// 	Some(Box::new(finality_proof_import)),
			// 	client,
			// 	inherent_data_providers.clone(),
			// 	spawn_task_handle,
			// 	prometheus_registry,
			// )?;
			let finality_proof_request_builder =
			Box::new(DummyFinalityProofRequestBuilder::default()) as Box<_>;

			let pow_block_import = sc_consensus_pow::PowBlockImport::new(
				client.clone(),
				client.clone(),
				crate::pow::Sha3Algorithm::new(client.clone()),
				0, // check_inherents_after,
				_select_chain,
				inherent_data_providers.clone(),
			);

			let import_queue = sc_consensus_pow::import_queue(
				Box::new(pow_block_import),
				None,
				None,
				crate::pow::Sha3Algorithm::new(client.clone()),
				inherent_data_providers.clone(),
				spawn_task_handle,
				prometheus_registry,
			)?;

			Ok((import_queue, finality_proof_request_builder))
		})?
		.with_finality_proof_provider(|client, backend| {
			// GenesisAuthoritySetProvider is implemented for StorageAndProofProvider
			let provider = client as Arc<dyn StorageAndProofProvider<_, _>>;
			Ok(Arc::new(GrandpaFinalityProofProvider::new(backend, provider)) as _)
		})?
		.build_light()
}
