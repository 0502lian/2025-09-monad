// Copyright (C) 2025 Category Labs, Inc.
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <http://www.gnu.org/licenses/>.

// 引入交易相关的核心类型定义
use alloy_consensus::{Transaction, TxEnvelope};
// 引入链级别的执行参数和链参数配置
use monad_chain_config::{execution_revision::ExecutionChainParams, revision::ChainParams};
// 引入交易验证过程中可能返回的错误类型
use monad_eth_txpool_types::TransactionError;

/// Stateless helper function to check validity of an Ethereum transaction
// 对以太坊交易进行静态验证的入口函数
pub fn static_validate_transaction(
    // 交易封装体的只读引用
    tx: &TxEnvelope,
    // 链的标识符
    chain_id: u64,
    // 链配置参数的只读引用
    chain_params: &ChainParams,
    // 执行环境配置参数的只读引用
    execution_chain_params: &ExecutionChainParams,
) -> Result<(), TransactionError> {
    // 如果交易属于 EIP-4844 类型则返回不支持错误
    if tx.is_eip4844() {
        return Err(TransactionError::UnsupportedTransactionType);
    }

    // 调用 TFM 校验器检查编码长度与 gas 限制
    TfmValidator::validate(tx, chain_params)?;

    // 以太坊 Homestead 硬分叉后的验证逻辑（包含 EIP-155）
    EthHomesteadForkValidation::validate(tx, chain_id)?;

    // 以太坊 London 硬分叉后的验证逻辑（包含 EIP-1559）
    EthLondonForkValidation::validate(tx)?;

    // 以太坊 Shanghai 硬分叉后的验证逻辑（包含 EIP-3860）
    EthShanghaiForkValidation::validate(tx, execution_chain_params)?;

    // 以太坊黄皮书中定义的固有 gas 验证
    YellowPaperValidation::validate(tx)?;

    // 以太坊 Prague 硬分叉后的验证逻辑（包含 EIP-7623 与 EIP-7702）
    EthPragueForkValidation::validate(tx, execution_chain_params)?;

    // 如果交易启用了 EIP-7702，则检查授权列表是否为空
    if tx.is_eip7702() {
        match tx.authorization_list() {
            // 授权列表存在时需要确保其中包含至少一个项目
            Some(auth_list) => {
                if auth_list.is_empty() {
                    return Err(TransactionError::AuthorizationListEmpty);
                }
            }
            // 授权列表缺失也视为错误
            None => return Err(TransactionError::AuthorizationListEmpty),
        }
    }

    // 所有验证均通过则返回 Ok
    Ok(())
}

// TFM 对 EIP-2718 编码交易允许的最大字节长度
pub const TFM_MAX_EIP2718_ENCODED_LENGTH: usize = 384 * 1024;
// TFM 允许的最大 gas 限制
pub const TFM_MAX_GAS_LIMIT: u64 = 30_000_000;
// EIP-7702 中每个空账户所需附加的 gas 成本
pub const EIP_7702_PER_EMPTY_ACCOUNT_COST: u64 = 25_000;

struct TfmValidator;
impl TfmValidator {
    // 验证交易是否满足 TFM 的基础约束
    fn validate(tx: &TxEnvelope, chain_params: &ChainParams) -> Result<(), TransactionError> {
        // 检查交易编码长度是否超过上限
        if tx.eip2718_encoded_length() > TFM_MAX_EIP2718_ENCODED_LENGTH {
            return Err(TransactionError::EncodedLengthLimitExceeded);
        }

        // 检查交易 gas 限制是否超过 TFM 设置的最大值
        if tx.gas_limit() > TFM_MAX_GAS_LIMIT {
            return Err(TransactionError::GasLimitTooHigh);
        }

        // 检查交易 gas 限制是否超过链配置中的提案 gas 上限
        if tx.gas_limit() > chain_params.proposal_gas_limit {
            return Err(TransactionError::GasLimitTooHigh);
        }

        // 所有检查通过则返回 Ok
        Ok(())
    }
}

struct YellowPaperValidation;
impl YellowPaperValidation {
    // 验证入口函数，复用固有 gas 校验逻辑
    fn validate(tx: &TxEnvelope) -> Result<(), TransactionError> {
        Self::intrinsic_gas_validation(tx)
    }

    // 按黄皮书公式计算并校验交易的固有 gas
    fn intrinsic_gas_validation(tx: &TxEnvelope) -> Result<(), TransactionError> {
        // YP 方程 62：计算固有 gas
        let intrinsic_gas = compute_intrinsic_gas(tx);
        // 如果交易的 gas 限制小于固有 gas，则返回过低错误
        if tx.gas_limit() < intrinsic_gas {
            return Err(TransactionError::GasLimitTooLow);
        }
        // 校验通过返回 Ok
        Ok(())
    }
}

struct EthHomesteadForkValidation;
impl EthHomesteadForkValidation {
    // 验证 Homestead 分叉相关的规则
    fn validate(tx: &TxEnvelope, chain_id: u64) -> Result<(), TransactionError> {
        // 先执行 EIP-2 相关签名检查
        Self::eip_2(tx)?;
        // 再执行 EIP-155 的链标识验证
        Self::eip_155(tx, chain_id)
    }

    // EIP-2：确保签名的 s 值位于曲线的下半区
    fn eip_2(tx: &TxEnvelope) -> Result<(), TransactionError> {
        // 如果 normalize_s 返回值，说明 s 不合法，需要报错
        if tx.signature().normalize_s().is_some() {
            return Err(TransactionError::UnsupportedTransactionType);
        }
        // 合法签名返回 Ok
        Ok(())
    }

    // EIP-155：验证交易链 ID 是否与本链一致
    fn eip_155(tx: &TxEnvelope, chain_id: u64) -> Result<(), TransactionError> {
        // 仍允许没有链 ID 的传统交易通过
        if let Some(tx_chain_id) = tx.chain_id() {
            if tx_chain_id != chain_id {
                return Err(TransactionError::InvalidChainId);
            }
        }
        // 链 ID 一致或不存在时返回 Ok
        Ok(())
    }
}

struct EthLondonForkValidation;
impl EthLondonForkValidation {
    // 验证 London 分叉相关的规则
    fn validate(tx: &TxEnvelope) -> Result<(), TransactionError> {
        Self::eip_1559(tx)
    }

    // EIP-1559：确保优先费不超过最大费率
    fn eip_1559(tx: &TxEnvelope) -> Result<(), TransactionError> {
        // 若交易定义了优先费，则进行比较
        if let Some(max_priority_fee) = tx.max_priority_fee_per_gas() {
            if max_priority_fee > tx.max_fee_per_gas() {
                return Err(TransactionError::MaxPriorityFeeTooHigh);
            }
        }
        // 校验通过返回 Ok
        Ok(())
    }
}

struct EthShanghaiForkValidation;
impl EthShanghaiForkValidation {
    // 验证 Shanghai 分叉相关的规则
    fn validate(
        // 当前交易引用
        tx: &TxEnvelope,
        // 执行链参数引用
        execution_chain_params: &ExecutionChainParams,
    ) -> Result<(), TransactionError> {
        Self::eip_3860(tx, execution_chain_params)
    }

    // EIP-3860：限制并度量合约创建的初始化代码长度
    fn eip_3860(
        // 当前交易引用
        tx: &TxEnvelope,
        // 执行链参数引用
        execution_chain_params: &ExecutionChainParams,
    ) -> Result<(), TransactionError> {
        // 最大 init_code 长度为 2 * max_code_size
        let max_init_code_size: usize = 2 * execution_chain_params.max_code_size;
        // 若交易为创建合约且输入长度超过限制则报错
        if tx.kind().is_create() && tx.input().len() > max_init_code_size {
            return Err(TransactionError::InitCodeLimitExceeded);
        }
        // 校验通过返回 Ok
        Ok(())
    }
}

struct EthPragueForkValidation;
impl EthPragueForkValidation {
    // 验证 Prague 分叉相关的规则
    fn validate(
        // 当前交易引用
        tx: &TxEnvelope,
        // 执行链参数引用
        execution_chain_params: &ExecutionChainParams,
    ) -> Result<(), TransactionError> {
        // 若 Prague 功能已启用则执行 EIP-7623 校验
        if execution_chain_params.prague_enabled {
            Self::eip_7623(tx)?;
        }
        // 始终执行 EIP-7702 相关校验
        Self::eip_7702(tx, execution_chain_params)?;

        // 校验通过返回 Ok
        Ok(())
    }

    // EIP-7623：计算数据最低 gas 并验证限制
    fn eip_7623(tx: &TxEnvelope) -> Result<(), TransactionError> {
        // 计算 EIP-7623 定义的数据最低 gas
        let floor_data_gas = compute_floor_data_gas(tx);
        // 若交易 gas 限制小于数据底线则报错
        if tx.gas_limit() < floor_data_gas {
            return Err(TransactionError::GasLimitTooLow);
        }
        // 校验通过返回 Ok
        Ok(())
    }

    // EIP-7702：验证授权列表及启用状态
    fn eip_7702(
        // 当前交易引用
        tx: &TxEnvelope,
        // 执行链参数引用
        execution_chain_params: &ExecutionChainParams,
    ) -> Result<(), TransactionError> {
        // 若交易未启用 EIP-7702，直接通过
        if !tx.is_eip7702() {
            return Ok(());
        }

        // 若链尚未启用 Prague，则视为不支持
        if !execution_chain_params.prague_enabled {
            return Err(TransactionError::UnsupportedTransactionType);
        }

        // 检查授权列表的存在性与非空性
        match tx.authorization_list() {
            Some(auth_list) => {
                if auth_list.is_empty() {
                    return Err(TransactionError::AuthorizationListEmpty);
                }
            }
            None => return Err(TransactionError::AuthorizationListEmpty),
        }

        // 校验通过返回 Ok
        Ok(())
    }
}

// 计算交易的固有 gas 数值
fn compute_intrinsic_gas(tx: &TxEnvelope) -> u64 {
    // 固定基础补贴 21000 gas
    let mut intrinsic_gas = 21000;

    // 黄皮书公式 60：统计输入中零字节与非零字节数量
    // 每个零字节消耗 4 gas，每个非零字节消耗 16 gas
    let zero_data_len = tx.input().iter().filter(|v| **v == 0).count() as u64;
    let non_zero_data_len = tx.input().len() as u64 - zero_data_len;
    intrinsic_gas += zero_data_len * 4;
    // EIP-2028：降低非零字节的 gas 成本到 16
    intrinsic_gas += non_zero_data_len * 16;

    if tx.kind().is_create() {
        // 若为合约创建交易则额外增加 32000 gas
        intrinsic_gas += 32000;
        // EIP-3860：为初始化代码解析提供补贴
        intrinsic_gas += ((tx.input().len() as u64 + 31) / 32) * 2;
    }

    // EIP-2930：处理访问列表带来的额外 gas 成本
    let access_list = tx
        .access_list()
        .map(|list| list.0.as_slice())
        .unwrap_or(&[]);
    let accessed_slots: usize = access_list.iter().map(|item| item.storage_keys.len()).sum();
    // 访问列表中的每个地址需要 2400 gas
    intrinsic_gas += access_list.len() as u64 * 2400;
    // 每个存储槽需要 1900 gas
    intrinsic_gas += accessed_slots as u64 * 1900;

    if tx.is_eip7702() {
        if let Some(auth_list) = tx.authorization_list() {
            // EIP-7702：为空账户的授权条目添加额外 gas，使用饱和加法避免溢出
            intrinsic_gas = intrinsic_gas.saturating_add(
                EIP_7702_PER_EMPTY_ACCOUNT_COST.saturating_mul(auth_list.len() as u64),
            );
        }
    }
    // 返回计算得到的固有 gas 值
    intrinsic_gas
}

// 根据 EIP-7623 计算交易数据的最低 gas 要求
fn compute_floor_data_gas(tx: &TxEnvelope) -> u64 {
    // 统计输入数据中的零字节数量
    let zero_data_len = tx.input().iter().filter(|v| **v == 0).count() as u64;
    let non_zero_data_len = tx.input().len() as u64 - zero_data_len;
    // 计算基础 21000 gas 与数据相关的额外开销
    21_000 + (zero_data_len * 10 + non_zero_data_len * 40)
}

#[cfg(test)]
mod test {
    use std::str::FromStr;

    use alloy_consensus::{SignableTransaction, TxEip1559, TxLegacy};
    use alloy_primitives::{hex, Address, Bytes, FixedBytes, PrimitiveSignature, TxKind, B256};
    use alloy_signer::SignerSync;
    use alloy_signer_local::PrivateKeySigner;
    use monad_chain_config::{
        execution_revision::MonadExecutionRevision, revision::MockChainRevision, ChainConfig,
        MockChainConfig,
    };
    use monad_eth_testutil::{make_eip7702_tx, make_signed_authorization, secret_to_eth_address};

    use super::*;

    // pubkey starts with AAA
    const S1: B256 = B256::new(hex!(
        "0ed2e19e3aca1a321349f295837988e9c6f95d4a6fc54cfab6befd5ee82662ad"
    ));
    // pubkey starts with BBB
    const S2: B256 = B256::new(hex!(
        "009ac901cf45a2e92e7e7bdf167dc52e3a6232be3c56cc3b05622b247c2c716a"
    ));

    const BASE_FEE: u64 = 100_000_000_000;

    fn chain_params_with_gas_limit(proposal_gas_limit: u64) -> ChainParams {
        ChainParams {
            tx_limit: MockChainRevision::DEFAULT.chain_params.tx_limit,
            proposal_gas_limit,
            proposal_byte_limit: MockChainRevision::DEFAULT.chain_params.proposal_byte_limit,
            vote_pace: MockChainRevision::DEFAULT.chain_params.vote_pace,
            max_reserve_balance: MockChainRevision::DEFAULT.chain_params.max_reserve_balance,
        }
    }

    fn sign_tx(signature_hash: &FixedBytes<32>) -> PrimitiveSignature {
        let secret_key = B256::repeat_byte(0xAu8).to_string();
        let signer = &secret_key.parse::<PrivateKeySigner>().unwrap();
        signer.sign_hash_sync(signature_hash).unwrap()
    }

    #[test]
    fn test_static_validate_transaction() {
        let address = Address(FixedBytes([0x11; 20]));

        let chain_id: u64 = MockChainConfig::DEFAULT.chain_id();

        // tx exceeds tfm encoded length limit
        let tx_exceeds_length_limit = TxLegacy {
            chain_id: None,
            nonce: 0,
            to: TxKind::Call(address),
            gas_price: 1000,
            gas_limit: 1_000_000,
            input: Bytes::from(vec![0u8;
                // 104: Magic number such that txn.eip2718_encoded_length() == TFM_MAX_EIP2718_ENCODED_LENGTH + 1
                 TFM_MAX_EIP2718_ENCODED_LENGTH - 104]),
            ..Default::default()
        };
        let signature = sign_tx(&tx_exceeds_length_limit.signature_hash());
        let txn = tx_exceeds_length_limit.into_signed(signature);
        assert_eq!(
            txn.eip2718_encoded_length(),
            TFM_MAX_EIP2718_ENCODED_LENGTH + 1
        );

        let result = static_validate_transaction(
            &txn.into(),
            chain_id,
            MockChainRevision::DEFAULT.chain_params,
            MonadExecutionRevision::LATEST.execution_chain_params(),
        );
        assert!(matches!(
            result,
            Err(TransactionError::EncodedLengthLimitExceeded)
        ));

        // tx exceeds tfm gas limit
        let tx_exceeds_length_limit = TxLegacy {
            chain_id: None,
            nonce: 0,
            to: TxKind::Call(address),
            gas_price: 1000,
            gas_limit: TFM_MAX_GAS_LIMIT + 1,
            ..Default::default()
        };
        let signature = sign_tx(&tx_exceeds_length_limit.signature_hash());
        let txn = tx_exceeds_length_limit.into_signed(signature);

        let result = static_validate_transaction(
            &txn.into(),
            chain_id,
            &chain_params_with_gas_limit(TFM_MAX_GAS_LIMIT + 2),
            MonadExecutionRevision::LATEST.execution_chain_params(),
        );
        assert!(matches!(result, Err(TransactionError::GasLimitTooHigh)));

        // transaction with gas limit higher than block gas limit
        let tx_gas_limit_too_high = TxEip1559 {
            chain_id,
            nonce: 0,
            to: TxKind::Call(address),
            max_fee_per_gas: 1000,
            max_priority_fee_per_gas: 10,
            gas_limit: TFM_MAX_GAS_LIMIT - 1,
            input: vec![].into(),
            ..Default::default()
        };
        let signature = sign_tx(&tx_gas_limit_too_high.signature_hash());
        let txn = tx_gas_limit_too_high.into_signed(signature);

        let result = static_validate_transaction(
            &txn.into(),
            chain_id,
            &chain_params_with_gas_limit(TFM_MAX_GAS_LIMIT - 2),
            MonadExecutionRevision::LATEST.execution_chain_params(),
        );
        assert!(matches!(result, Err(TransactionError::GasLimitTooHigh)));

        // pre EIP-155 transaction with no chain id is allowed
        let tx_no_chain_id = TxLegacy {
            chain_id: None,
            nonce: 0,
            to: TxKind::Call(address),
            gas_price: 1000,
            gas_limit: 1_000_000,
            ..Default::default()
        };
        let signature = sign_tx(&tx_no_chain_id.signature_hash());
        let txn = tx_no_chain_id.into_signed(signature);

        let result = static_validate_transaction(
            &txn.into(),
            chain_id,
            MockChainRevision::DEFAULT.chain_params,
            MonadExecutionRevision::LATEST.execution_chain_params(),
        );
        assert!(matches!(result, Ok(())));

        // transaction with incorrect chain id
        let tx_invalid_chain_id = TxEip1559 {
            chain_id: chain_id - 1,
            nonce: 0,
            to: TxKind::Call(address),
            max_fee_per_gas: 1000,
            max_priority_fee_per_gas: 10,
            gas_limit: 1_000_000,
            ..Default::default()
        };
        let signature = sign_tx(&tx_invalid_chain_id.signature_hash());
        let txn = tx_invalid_chain_id.into_signed(signature);

        let result = static_validate_transaction(
            &txn.into(),
            chain_id,
            MockChainRevision::DEFAULT.chain_params,
            MonadExecutionRevision::LATEST.execution_chain_params(),
        );
        assert!(matches!(result, Err(TransactionError::InvalidChainId)));

        // contract deployment transaction with input data larger than 2 * max_code_size (initcode limit)
        let input = vec![
            0;
            2 * MonadExecutionRevision::LATEST
                .execution_chain_params()
                .max_code_size
                + 1
        ];
        let tx_over_initcode_limit = TxEip1559 {
            chain_id,
            nonce: 0,
            to: TxKind::Create,
            max_fee_per_gas: 10000,
            max_priority_fee_per_gas: 10,
            gas_limit: 1_000_000,
            input: input.into(),
            ..Default::default()
        };
        let signature = sign_tx(&tx_over_initcode_limit.signature_hash());
        let txn = tx_over_initcode_limit.into_signed(signature);

        let result = static_validate_transaction(
            &txn.into(),
            chain_id,
            MockChainRevision::DEFAULT.chain_params,
            MonadExecutionRevision::LATEST.execution_chain_params(),
        );
        assert!(matches!(
            result,
            Err(TransactionError::InitCodeLimitExceeded)
        ));

        // transaction with larger max priority fee than max fee per gas
        let tx_priority_fee_too_high = TxEip1559 {
            chain_id,
            nonce: 0,
            to: TxKind::Call(address),
            max_fee_per_gas: 1000,
            max_priority_fee_per_gas: 10000,
            gas_limit: 1_000_000,
            input: vec![].into(),
            ..Default::default()
        };
        let signature = sign_tx(&tx_priority_fee_too_high.signature_hash());
        let txn = tx_priority_fee_too_high.into_signed(signature);

        let result = static_validate_transaction(
            &txn.into(),
            chain_id,
            MockChainRevision::DEFAULT.chain_params,
            MonadExecutionRevision::LATEST.execution_chain_params(),
        );
        assert!(matches!(
            result,
            Err(TransactionError::MaxPriorityFeeTooHigh)
        ));

        // transaction with gas limit lower than intrinsic gas
        let tx_gas_limit_too_low = TxEip1559 {
            chain_id,
            nonce: 0,
            to: TxKind::Call(address),
            max_fee_per_gas: 1000,
            max_priority_fee_per_gas: 10,
            gas_limit: 20_000,
            input: vec![].into(),
            ..Default::default()
        };
        let signature = sign_tx(&tx_gas_limit_too_low.signature_hash());
        let txn = tx_gas_limit_too_low.into_signed(signature);

        let result = static_validate_transaction(
            &txn.into(),
            chain_id,
            MockChainRevision::DEFAULT.chain_params,
            MonadExecutionRevision::LATEST.execution_chain_params(),
        );
        assert!(matches!(result, Err(TransactionError::GasLimitTooLow)));

        // transaction with gas limit lower than floor data gas
        // floor data gas is 21000 + (zero byte * 10) + (non-zero byte * 40)
        let tx_gas_limit_too_low_data = TxEip1559 {
            chain_id,
            nonce: 0,
            to: TxKind::Call(address),
            max_fee_per_gas: 1000,
            max_priority_fee_per_gas: 10,
            gas_limit: 30_000,
            input: vec![0xaa; 226].into(), // 21000 + (226 * 40) > 30000
            ..Default::default()
        };
        let signature = sign_tx(&tx_gas_limit_too_low_data.signature_hash());
        let txn = tx_gas_limit_too_low_data.into_signed(signature);

        let result = static_validate_transaction(
            &txn.into(),
            chain_id,
            MockChainRevision::DEFAULT.chain_params,
            MonadExecutionRevision::LATEST.execution_chain_params(),
        );
        assert!(matches!(result, Err(TransactionError::GasLimitTooLow)));
    }

    #[test]
    fn test_compute_floor_data_gas() {
        const CHAIN_ID: u64 = 1337;
        let tx = TxEip1559 {
            chain_id: CHAIN_ID,
            nonce: 0,
            to: TxKind::Call(Address(FixedBytes([0x11; 20]))),
            max_fee_per_gas: 1000,
            max_priority_fee_per_gas: 10,
            gas_limit: 1_000_000,
            // input data with 3 zero byte and 4 non-zero byte
            input: Bytes::from_str("0x12003456000078").unwrap(),
            ..Default::default()
        };
        let signature = sign_tx(&tx.signature_hash());
        let tx = tx.into_signed(signature);

        let result = compute_floor_data_gas(&tx.into());
        assert_eq!(result, 21000 + (3 * 10) + (4 * 40));
    }

    #[test]
    fn test_compute_intrinsic_gas() {
        const CHAIN_ID: u64 = 1337;
        let tx = TxEip1559 {
            chain_id: CHAIN_ID,
            nonce: 0,
            to: TxKind::Create,
            max_fee_per_gas: 1000,
            max_priority_fee_per_gas: 10,
            gas_limit: 1_000_000,
            input: Bytes::from_str("0x6040608081523462000414").unwrap(),
            ..Default::default()
        };
        let signature = sign_tx(&tx.signature_hash());
        let tx = tx.into_signed(signature);

        let result = compute_intrinsic_gas(&tx.into());
        assert_eq!(result, 53166);
    }

    #[test]
    fn test_compute_intrinsic_gas_eip7702() {
        let tx_1_auth = make_eip7702_tx(
            S1,
            BASE_FEE as u128,
            0,
            100_000,
            0,
            vec![make_signed_authorization(S2, secret_to_eth_address(S1), 0)],
            0,
        );

        let result_1_auth = compute_intrinsic_gas(&tx_1_auth);
        assert_eq!(result_1_auth, 46000);

        let tx_2_auth = make_eip7702_tx(
            S1,
            BASE_FEE as u128,
            0,
            100_000,
            0,
            vec![
                make_signed_authorization(S2, secret_to_eth_address(S1), 0),
                make_signed_authorization(S2, secret_to_eth_address(S1), 0),
            ],
            0,
        );

        let result_2_auth = compute_intrinsic_gas(&tx_2_auth);
        assert_eq!(result_2_auth, 71000);
    }
}
