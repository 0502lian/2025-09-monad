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

// 使用标准库中的集合、标记类型以及范围操作。
// 中文注释: 参见下一行代码含义
use std::{
    // 引入有序映射和哈希集合以跟踪账户状态。
    // 中文注释: 参见下一行代码含义
    collections::{BTreeMap, HashSet},
    // 引入 PhantomData 用于保存泛型类型信息。
    // 中文注释: 参见下一行代码含义
    marker::PhantomData,
    // 引入 Deref、Range、RangeFrom 以便处理引用和区间。
    // 中文注释: 参见下一行代码含义
    ops::{Deref, Range, RangeFrom},
// 中文注释: 参见下一行代码含义
};

// 引入共识层定义的交易相关类型及封装格式。
// 中文注释: 参见下一行代码含义
use alloy_consensus::{
    // Recovered 和 Transaction 用于处理已恢复签名的交易。
    // 中文注释: 参见下一行代码含义
    transaction::{Recovered, Transaction},
    // TxEnvelope 代表通用交易封装。
    // 中文注释: 参见下一行代码含义
    TxEnvelope,
// 中文注释: 参见下一行代码含义
};
// 引入 EIP-7702 授权恢复结构。
// 中文注释: 参见下一行代码含义
use alloy_eips::eip7702::RecoveredAuthorization;
// 引入地址、交易哈希及大整数类型。
// 中文注释: 参见下一行代码含义
use alloy_primitives::{Address, TxHash, U256};
// 引入迭代器工具用于集合操作。
// 中文注释: 参见下一行代码含义
use itertools::Itertools;
// 引入链配置及修订相关类型。
// 中文注释: 参见下一行代码含义
use monad_chain_config::{
    // 中文注释: 参见下一行代码含义
    execution_revision::MonadExecutionRevision, revision::ChainRevision, ChainConfig,
// 中文注释: 参见下一行代码含义
};
// 引入共识块、账户余额、策略等核心类型。
// 中文注释: 参见下一行代码含义
use monad_consensus_types::{
    // 中文注释: 参见下一行代码含义
    block::{
        // 中文注释: 参见下一行代码含义
        AccountBalanceState, BlockPolicy, BlockPolicyBlockValidator,
        // 中文注释: 参见下一行代码含义
        BlockPolicyBlockValidatorError, BlockPolicyError, ConsensusFullBlock, TxnFee, TxnFees,
    // 中文注释: 参见下一行代码含义
    },
    // 引入区块树根信息结构。
    // 中文注释: 参见下一行代码含义
    checkpoint::RootInfo,
// 中文注释: 参见下一行代码含义
};
// 引入证书签名公钥与可恢复签名能力。
// 中文注释: 参见下一行代码含义
use monad_crypto::certificate_signature::{
    // 中文注释: 参见下一行代码含义
    CertificateSignaturePubKey, CertificateSignatureRecoverable,
// 中文注释: 参见下一行代码含义
};
// 引入以太坊账户、执行协议、区块头与验证交易类型。
// 中文注释: 参见下一行代码含义
use monad_eth_types::{EthAccount, EthExecutionProtocol, EthHeader, ValidatedTx};
// 引入状态后端接口及错误类型。
// 中文注释: 参见下一行代码含义
use monad_state_backend::{StateBackend, StateBackendError};
// 引入系统交易类型。
// 中文注释: 参见下一行代码含义
use monad_system_calls::SystemTransaction;
// 引入与余额、区块、轮次等相关的基本类型常量。
// 中文注释: 参见下一行代码含义
use monad_types::{
    // 中文注释: 参见下一行代码含义
    Balance, BlockId, Epoch, Nonce, Round, SeqNum, GENESIS_BLOCK_ID, GENESIS_ROUND, GENESIS_SEQ_NUM,
// 中文注释: 参见下一行代码含义
};
// 引入签名集合接口。
// 中文注释: 参见下一行代码含义
use monad_validator::signature_collection::SignatureCollection;
// 引入按序向量映射用于缓存。
// 中文注释: 参见下一行代码含义
use sorted_vector_map::SortedVectorMap;
// 引入日志宏以便调试跟踪。
// 中文注释: 参见下一行代码含义
use tracing::{debug, trace, warn};

// 引入本模块中的 nonce 使用记录结构。
// 中文注释: 参见下一行代码含义
use self::nonce_usage::{NonceUsage, NonceUsageMap};

// 暴露 nonce 使用相关子模块。
// 中文注释: 参见下一行代码含义
pub mod nonce_usage;
// 暴露验证相关子模块。
// 中文注释: 参见下一行代码含义
pub mod validation;

// 定义保留余额检查的阶段枚举。
// 中文注释: 参见下一行代码含义
pub enum ReserveBalanceCheck {
    // 插入阶段需要检查保留余额。
    // 中文注释: 参见下一行代码含义
    Insert,
    // 提案阶段需要检查保留余额。
    // 中文注释: 参见下一行代码含义
    Propose,
    // 验证阶段需要检查保留余额。
    // 中文注释: 参见下一行代码含义
    Validate,
// 中文注释: 参见下一行代码含义
}

// 在 TFM 启用前计算交易可能消耗的最大金额。
// 中文注释: 参见下一行代码含义
pub fn pre_tfm_compute_max_txn_cost(txn: &TxEnvelope) -> U256 {
    // 读取交易本身的数额。
    // 中文注释: 参见下一行代码含义
    let txn_value = txn.value();
    // 将 gas 上限转换为 U256。
    // 中文注释: 参见下一行代码含义
    let gas_limit = U256::from(txn.gas_limit());
    // 获取最高 gas 单价并转换为 U256。
    // 中文注释: 参见下一行代码含义
    let max_fee = U256::from(txn.max_fee_per_gas());
    // 计算 gas 费用并确保不溢出。
    // 中文注释: 参见下一行代码含义
    let max_gas_cost = gas_limit.checked_mul(max_fee).expect("no overflow");
    // 返回交易数额与 gas 费用之和，使用饱和加防止溢出。
    // 中文注释: 参见下一行代码含义
    txn_value.saturating_add(max_gas_cost)
// 中文注释: 参见下一行代码含义
}

// 计算在 TFM 启用后交易可能消耗的最大总价值。
// 中文注释: 参见下一行代码含义
pub fn compute_txn_max_value(txn: &TxEnvelope, base_fee: u64) -> U256 {
    // 读取交易数额。
    // 中文注释: 参见下一行代码含义
    let txn_value = txn.value();
    // 计算最大 gas 花费。
    // 中文注释: 参见下一行代码含义
    let gas_cost = compute_txn_max_gas_cost(txn, base_fee);
    // 返回数额与 gas 费用之和。
    // 中文注释: 参见下一行代码含义
    txn_value.saturating_add(gas_cost)
// 中文注释: 参见下一行代码含义
}

// 根据 base fee 计算交易最大可能的 gas 成本。
// 中文注释: 参见下一行代码含义
pub fn compute_txn_max_gas_cost(txn: &TxEnvelope, base_fee: u64) -> U256 {
    // 将 gas 上限转成 U256。
    // 中文注释: 参见下一行代码含义
    let gas_limit = U256::from(txn.gas_limit());
    // 取最高报价。
    // 中文注释: 参见下一行代码含义
    let max_fee = U256::from(txn.max_fee_per_gas());
    // 获取最大优先费，缺省为零。
    // 中文注释: 参见下一行代码含义
    let priority_fee = U256::from(txn.max_priority_fee_per_gas().unwrap_or(0));
    // 将 base fee 转为 U256。
    // 中文注释: 参见下一行代码含义
    let base_fee = U256::from(base_fee);
    // 实际 gas 出价为最高出价与 base fee 加优先费中的较小者。
    // 中文注释: 参见下一行代码含义
    let gas_bid = max_fee.min(base_fee.saturating_add(priority_fee));
    // 计算最终花费并确保不溢出。
    // 中文注释: 参见下一行代码含义
    gas_limit.checked_mul(gas_bid).expect("no overflow")
// 中文注释: 参见下一行代码含义
}

// 用于在状态后端中定位区块执行结果的索引结构。
// 中文注释: 参见下一行代码含义
struct BlockLookupIndex {
    // 对应区块的唯一标识。
    // 中文注释: 参见下一行代码含义
    block_id: BlockId,
    // 区块的序号。
    // 中文注释: 参见下一行代码含义
    seq_num: SeqNum,
    // 区块所属的轮次。
    // 中文注释: 参见下一行代码含义
    round: Round,
    // 指示该区块是否已经最终确认。
    // 中文注释: 参见下一行代码含义
    is_finalized: bool,
// 中文注释: 参见下一行代码含义
}

/// A consensus block that has gone through the EthereumValidator and makes the decoded and
/// verified transactions available to access
// 代表通过以太坊验证器的共识区块，携带验证后的交易。
// 中文注释: 参见下一行代码含义
#[derive(Debug, Clone)]
// 中文注释: 参见下一行代码含义
pub struct EthValidatedBlock<ST, SCT>
// 中文注释: 参见下一行代码含义
where
    // 中文注释: 参见下一行代码含义
    ST: CertificateSignatureRecoverable,
    // 中文注释: 参见下一行代码含义
    SCT: SignatureCollection<NodeIdPubKey = CertificateSignaturePubKey<ST>>,
// 中文注释: 参见下一行代码含义
{
    // 原始共识区块。
    // 中文注释: 参见下一行代码含义
    pub block: ConsensusFullBlock<ST, SCT, EthExecutionProtocol>,
    // 区块内包含的系统交易集合。
    // 中文注释: 参见下一行代码含义
    pub system_txns: Vec<SystemTransaction>,
    // 已验证并恢复签名的用户交易。
    // 中文注释: 参见下一行代码含义
    pub validated_txns: Vec<ValidatedTx>,
    // 交易所涉及的 nonce 使用记录。
    // 中文注释: 参见下一行代码含义
    pub nonce_usages: NonceUsageMap,
    // 交易费用明细。
    // 中文注释: 参见下一行代码含义
    pub txn_fees: TxnFees,
// 中文注释: 参见下一行代码含义
}

// 中文注释: 参见下一行代码含义
impl<ST, SCT> AsRef<EthValidatedBlock<ST, SCT>> for EthValidatedBlock<ST, SCT>
// 中文注释: 参见下一行代码含义
where
    // 中文注释: 参见下一行代码含义
    ST: CertificateSignatureRecoverable,
    // 中文注释: 参见下一行代码含义
    SCT: SignatureCollection<NodeIdPubKey = CertificateSignaturePubKey<ST>>,
// 中文注释: 参见下一行代码含义
{
    // 提供引用访问自身的便捷实现。
    // 中文注释: 参见下一行代码含义
    fn as_ref(&self) -> &EthValidatedBlock<ST, SCT> {
        // 中文注释: 参见下一行代码含义
        self
    // 中文注释: 参见下一行代码含义
    }
// 中文注释: 参见下一行代码含义
}

// 中文注释: 参见下一行代码含义
impl<ST, SCT> Deref for EthValidatedBlock<ST, SCT>
// 中文注释: 参见下一行代码含义
where
    // 中文注释: 参见下一行代码含义
    ST: CertificateSignatureRecoverable,
    // 中文注释: 参见下一行代码含义
    SCT: SignatureCollection<NodeIdPubKey = CertificateSignaturePubKey<ST>>,
// 中文注释: 参见下一行代码含义
{
    // 中文注释: 参见下一行代码含义
    type Target = ConsensusFullBlock<ST, SCT, EthExecutionProtocol>;
    // 允许直接将结构体视为内部的共识区块。
    // 中文注释: 参见下一行代码含义
    fn deref(&self) -> &Self::Target {
        // 中文注释: 参见下一行代码含义
        &self.block
    // 中文注释: 参见下一行代码含义
    }
// 中文注释: 参见下一行代码含义
}

// 中文注释: 参见下一行代码含义
impl<ST, SCT> EthValidatedBlock<ST, SCT>
// 中文注释: 参见下一行代码含义
where
    // 中文注释: 参见下一行代码含义
    ST: CertificateSignatureRecoverable,
    // 中文注释: 参见下一行代码含义
    SCT: SignatureCollection<NodeIdPubKey = CertificateSignaturePubKey<ST>>,
// 中文注释: 参见下一行代码含义
{
    // 收集区块中所有已验证交易的哈希。
    // 中文注释: 参见下一行代码含义
    pub fn get_validated_txn_hashes(&self) -> Vec<TxHash> {
        // 中文注释: 参见下一行代码含义
        self.validated_txns.iter().map(|t| *t.tx_hash()).collect()
    // 中文注释: 参见下一行代码含义
    }

    // 统计区块中所有已验证交易的总 gas 使用量。
    // 中文注释: 参见下一行代码含义
    pub fn get_total_gas(&self) -> u64 {
        // 中文注释: 参见下一行代码含义
        self.validated_txns
            // 中文注释: 参见下一行代码含义
            .iter()
            // 中文注释: 参见下一行代码含义
            .fold(0, |acc, tx| acc + tx.gas_limit())
    // 中文注释: 参见下一行代码含义
    }
// 中文注释: 参见下一行代码含义
}

// 中文注释: 参见下一行代码含义
impl<ST, SCT> PartialEq for EthValidatedBlock<ST, SCT>
// 中文注释: 参见下一行代码含义
where
    // 中文注释: 参见下一行代码含义
    ST: CertificateSignatureRecoverable,
    // 中文注释: 参见下一行代码含义
    SCT: SignatureCollection<NodeIdPubKey = CertificateSignaturePubKey<ST>>,
// 中文注释: 参见下一行代码含义
{
    // 中文注释: 参见下一行代码含义
    fn eq(&self, other: &Self) -> bool {
        // 中文注释: 参见下一行代码含义
        self.block == other.block
    // 中文注释: 参见下一行代码含义
    }
// 中文注释: 参见下一行代码含义
}
// 中文注释: 参见下一行代码含义
impl<ST, SCT> Eq for EthValidatedBlock<ST, SCT>
// 中文注释: 参见下一行代码含义
where
    // 中文注释: 参见下一行代码含义
    ST: CertificateSignatureRecoverable,
    // 中文注释: 参见下一行代码含义
    SCT: SignatureCollection<NodeIdPubKey = CertificateSignaturePubKey<ST>>,
// 中文注释: 参见下一行代码含义
{
// 中文注释: 参见下一行代码含义
}

// 缓存区块内各账户的交易费用信息。
// 中文注释: 参见下一行代码含义
#[derive(Debug)]
// 中文注释: 参见下一行代码含义
struct BlockTxnFeeStates {
    // 每个地址对应的交易费用。
    // 中文注释: 参见下一行代码含义
    txn_fees: TxnFees,
// 中文注释: 参见下一行代码含义
}

// 中文注释: 参见下一行代码含义
impl BlockTxnFeeStates {
    // 根据地址获取费用记录。
    // 中文注释: 参见下一行代码含义
    fn get(&self, eth_address: &Address) -> Option<TxnFee> {
        // 中文注释: 参见下一行代码含义
        self.txn_fees.get(eth_address).cloned()
    // 中文注释: 参见下一行代码含义
    }
// 中文注释: 参见下一行代码含义
}

// 已提交区块的缓存记录。
// 中文注释: 参见下一行代码含义
#[derive(Debug)]
// 中文注释: 参见下一行代码含义
struct CommittedBlock {
    // 区块标识。
    // 中文注释: 参见下一行代码含义
    block_id: BlockId,
    // 区块轮次。
    // 中文注释: 参见下一行代码含义
    round: Round,
    // 所属纪元。
    // 中文注释: 参见下一行代码含义
    epoch: Epoch,
    // 区块序号。
    // 中文注释: 参见下一行代码含义
    seq_num: SeqNum,
    // 记录 nonce 使用情况。
    // 中文注释: 参见下一行代码含义
    nonce_usages: NonceUsageMap,
    // 区块时间戳（纳秒）。
    // 中文注释: 参见下一行代码含义
    timestamp_ns: u128,
    // 区块中产生的交易费用。
    // 中文注释: 参见下一行代码含义
    fees: BlockTxnFeeStates,

    // 区块的 base fee 字段。
    // 中文注释: 参见下一行代码含义
    base_fee: Option<u64>,
    // base fee 的趋势值。
    // 中文注释: 参见下一行代码含义
    base_fee_trend: Option<u64>,
    // base fee 的动量值。
    // 中文注释: 参见下一行代码含义
    base_fee_moment: Option<u64>,
    // 区块的总 gas 使用量。
    // 中文注释: 参见下一行代码含义
    block_gas_usage: u64,
// 中文注释: 参见下一行代码含义
}

// 用于缓存最近提交区块的循环缓冲区。
// 中文注释: 参见下一行代码含义
#[derive(Debug)]
// 中文注释: 参见下一行代码含义
struct CommittedBlkBuffer<ST, SCT, CCT, CRT> {
    // 以区块序号为键存储提交区块。
    // 中文注释: 参见下一行代码含义
    blocks: SortedVectorMap<SeqNum, CommittedBlock>,
    // 缓冲区的最小容量，通常为执行延迟的两倍。
    // 中文注释: 参见下一行代码含义
    min_buffer_size: usize, // should be 2 * execution delay

    // PhantomData 用于保持泛型生命周期。
    // 中文注释: 参见下一行代码含义
    _phantom: PhantomData<(ST, SCT, fn(&CCT, &CRT))>,
// 中文注释: 参见下一行代码含义
}

// 中文注释: 参见下一行代码含义
impl<ST, SCT, CCT, CRT> CommittedBlkBuffer<ST, SCT, CCT, CRT>
// 中文注释: 参见下一行代码含义
where
    // 中文注释: 参见下一行代码含义
    ST: CertificateSignatureRecoverable,
    // 中文注释: 参见下一行代码含义
    SCT: SignatureCollection<NodeIdPubKey = CertificateSignaturePubKey<ST>>,
    // 中文注释: 参见下一行代码含义
    CCT: ChainConfig<CRT>,
    // 中文注释: 参见下一行代码含义
    CRT: ChainRevision,
// 中文注释: 参见下一行代码含义
{
    // 中文注释: 参见下一行代码含义
    fn new(min_buffer_size: usize) -> Self {
        // 初始化缓存结构并记录最小容量。
        // 中文注释: 参见下一行代码含义
        Self {
            // 中文注释: 参见下一行代码含义
            blocks: Default::default(),
            // 中文注释: 参见下一行代码含义
            min_buffer_size,

            // 中文注释: 参见下一行代码含义
            _phantom: Default::default(),
        // 中文注释: 参见下一行代码含义
        }
    // 中文注释: 参见下一行代码含义
    }

    // 中文注释: 参见下一行代码含义
    fn get_epoch(&self, seq_num: SeqNum) -> Option<Epoch> {
        // 根据区块序号查询纪元信息。
        // 中文注释: 参见下一行代码含义
        self.blocks
            // 中文注释: 参见下一行代码含义
            .get(&seq_num)
            // 中文注释: 参见下一行代码含义
            .map(|committed_block| committed_block.epoch)
    // 中文注释: 参见下一行代码含义
    }

    // 中文注释: 参见下一行代码含义
    fn update_account_balance(
        // 中文注释: 参见下一行代码含义
        &self,
        // 中文注释: 参见下一行代码含义
        account_balance: &mut AccountBalanceState,
        // 中文注释: 参见下一行代码含义
        eth_address: &Address,
        // 中文注释: 参见下一行代码含义
        execution_delay: SeqNum,
        // 中文注释: 参见下一行代码含义
        emptying_txn_check_block_range: Range<SeqNum>,
        // 中文注释: 参见下一行代码含义
        reserve_balance_check_block_range: RangeFrom<SeqNum>,
        // 中文注释: 参见下一行代码含义
        chain_config: &CCT,
    // 中文注释: 参见下一行代码含义
    ) -> Result<SeqNum, BlockPolicyError> {
        // 中文注释: 参见下一行代码含义
        trace!(
            // 中文注释: 参见下一行代码含义
            ?emptying_txn_check_block_range,
            // 中文注释: 参见下一行代码含义
            ?reserve_balance_check_block_range,
            // 中文注释: 参见下一行代码含义
            ?account_balance,
            // 中文注释: 参见下一行代码含义
            ?eth_address,
            // 中文注释: 参见下一行代码含义
            "before update_account_balance"
        // 中文注释: 参见下一行代码含义
        );

        // 中文注释: 参见下一行代码含义
        let mut next_validate = emptying_txn_check_block_range.start;
        // 中文注释: 参见下一行代码含义
        for (seq_num, block) in self.blocks.range(emptying_txn_check_block_range) {
            // 确认遍历区间连续。
            // 中文注释: 参见下一行代码含义
            assert_eq!(*seq_num, next_validate, "Emptying range is not contiguous");

            // 中文注释: 参见下一行代码含义
            if block.fees.get(eth_address).is_some()
                // 中文注释: 参见下一行代码含义
                && account_balance.block_seqnum_of_latest_txn < block.seq_num
            // 中文注释: 参见下一行代码含义
            {
                // 有费用产生时更新最近交易区块序号。
                // 中文注释: 参见下一行代码含义
                account_balance.block_seqnum_of_latest_txn = block.seq_num;
            // 中文注释: 参见下一行代码含义
            }
            // 中文注释: 参见下一行代码含义
            next_validate += SeqNum(1);
        // 中文注释: 参见下一行代码含义
        }

        // 中文注释: 参见下一行代码含义
        for (seq_num, block) in self.blocks.range(reserve_balance_check_block_range) {
            // 中文注释: 参见下一行代码含义
            assert_eq!(
                // 中文注释: 参见下一行代码含义
                *seq_num, next_validate,
                // 中文注释: 参见下一行代码含义
                "Reserve balance check range is not contiguous"
            // 中文注释: 参见下一行代码含义
            );

            // 中文注释: 参见下一行代码含义
            if let Some(block_txn_fees) = block.fees.get(eth_address) {
                // 构建区块验证器以应用费用变化。
                // 中文注释: 参见下一行代码含义
                let validator = EthBlockPolicyBlockValidator::new(
                    // 中文注释: 参见下一行代码含义
                    block.seq_num,
                    // 中文注释: 参见下一行代码含义
                    execution_delay,
                    // 中文注释: 参见下一行代码含义
                    block
                        // 中文注释: 参见下一行代码含义
                        .base_fee
                        // 中文注释: 参见下一行代码含义
                        .unwrap_or(monad_tfm::base_fee::PRE_TFM_BASE_FEE),
                    // 中文注释: 参见下一行代码含义
                    &chain_config.get_chain_revision(block.round),
                    // 中文注释: 参见下一行代码含义
                    &chain_config
                        // 中文注释: 参见下一行代码含义
                        .get_execution_chain_revision(timestamp_ns_to_secs(block.timestamp_ns)),
                // 中文注释: 参见下一行代码含义
                )?;
                // 中文注释: 参见下一行代码含义
                trace!(
                    // 中文注释: 参见下一行代码含义
                    "applying fees for block {:?}, curr acc balance: {:?}",
                    // 中文注释: 参见下一行代码含义
                    block.seq_num,
                    // 中文注释: 参见下一行代码含义
                    account_balance
                // 中文注释: 参见下一行代码含义
                );
                // 更新账户余额状态。
                // 中文注释: 参见下一行代码含义
                validator.try_apply_block_fees(account_balance, &block_txn_fees, eth_address)?;
            // 中文注释: 参见下一行代码含义
            }
            // 中文注释: 参见下一行代码含义
            next_validate += SeqNum(1);
        // 中文注释: 参见下一行代码含义
        }

        // 中文注释: 参见下一行代码含义
        trace!(
            // 中文注释: 参见下一行代码含义
            ?account_balance,
            // 中文注释: 参见下一行代码含义
            ?eth_address,
            // 中文注释: 参见下一行代码含义
            "after update_account_balance"
        // 中文注释: 参见下一行代码含义
        );

        // 中文注释: 参见下一行代码含义
        Ok(next_validate)
    // 中文注释: 参见下一行代码含义
    }

    // 中文注释: 参见下一行代码含义
    fn update_committed_block(&mut self, block: &EthValidatedBlock<ST, SCT>) {
        // 中文注释: 参见下一行代码含义
        let block_number = block.get_seq_num();
        // 中文注释: 参见下一行代码含义
        debug!(?block_number, ?block.txn_fees, "update_committed_block");
        // 中文注释: 参见下一行代码含义
        if let Some((&last_block_num, _)) = self.blocks.last_key_value() {
            // 新区块必须紧随缓存中的最后一个区块。
            // 中文注释: 参见下一行代码含义
            assert_eq!(last_block_num + SeqNum(1), block_number);
        // 中文注释: 参见下一行代码含义
        }

        // 中文注释: 参见下一行代码含义
        let current_size = self.blocks.len();

        // 中文注释: 参见下一行代码含义
        if current_size >= self.min_buffer_size.saturating_mul(2) {
            // 当缓存超过两倍容量时，丢弃最早的区块。
            // 中文注释: 参见下一行代码含义
            let (&first_block_num, _) = self.blocks.first_key_value().expect("txns non-empty");
            // 中文注释: 参见下一行代码含义
            let divider =
                // 中文注释: 参见下一行代码含义
                first_block_num + SeqNum(current_size as u64 - self.min_buffer_size as u64);

            // TODO: revisit once perf implications are understood
            // 中文注释: 参见下一行代码含义
            self.blocks = self.blocks.split_off(&divider);
            // 断言保留的新缓存依然连续。
            // 中文注释: 参见下一行代码含义
            assert_eq!(
                // 中文注释: 参见下一行代码含义
                *self.blocks.last_key_value().expect("non-empty").0 + SeqNum(1),
                // 中文注释: 参见下一行代码含义
                block_number
            // 中文注释: 参见下一行代码含义
            );
            // 确认当前缓存不少于最小容量。
            // 中文注释: 参见下一行代码含义
            assert!(self.blocks.len() >= self.min_buffer_size);
        // 中文注释: 参见下一行代码含义
        }

        // 计算区块总 gas 用于后续缓存。
        // 中文注释: 参见下一行代码含义
        let block_gas_usage = block.get_total_gas();

        // 中文注释: 参见下一行代码含义
        assert!(self
            // 中文注释: 参见下一行代码含义
            .blocks
            // 中文注释: 参见下一行代码含义
            .insert(
                // 中文注释: 参见下一行代码含义
                block_number,
                // 中文注释: 参见下一行代码含义
                CommittedBlock {
                    // 中文注释: 参见下一行代码含义
                    block_id: block.get_id(),
                    // 中文注释: 参见下一行代码含义
                    round: block.get_block_round(),
                    // 中文注释: 参见下一行代码含义
                    epoch: block.get_epoch(),
                    // 中文注释: 参见下一行代码含义
                    seq_num: block.get_seq_num(),
                    // 中文注释: 参见下一行代码含义
                    nonce_usages: block.nonce_usages.clone(),
                    // 中文注释: 参见下一行代码含义
                    timestamp_ns: block.get_timestamp(),
                    // 中文注释: 参见下一行代码含义
                    fees: BlockTxnFeeStates {
                        // 中文注释: 参见下一行代码含义
                        txn_fees: block.txn_fees.clone()
                    // 中文注释: 参见下一行代码含义
                    },

                    // 中文注释: 参见下一行代码含义
                    base_fee: block.block.header().base_fee,
                    // 中文注释: 参见下一行代码含义
                    base_fee_trend: block.block.header().base_fee_trend,
                    // 中文注释: 参见下一行代码含义
                    base_fee_moment: block.block.header().base_fee_moment,
                    // 中文注释: 参见下一行代码含义
                    block_gas_usage,
                // 中文注释: 参见下一行代码含义
                },
            // 中文注释: 参见下一行代码含义
            )
            // 中文注释: 参见下一行代码含义
            .is_none());
    // 中文注释: 参见下一行代码含义
    }
// 中文注释: 参见下一行代码含义
}

// 中文注释: 参见下一行代码含义
pub struct EthBlockPolicyBlockValidator<CRT>
// 中文注释: 参见下一行代码含义
where
    // 中文注释: 参见下一行代码含义
    CRT: ChainRevision,
// 中文注释: 参见下一行代码含义
{
    // 当前验证的区块序号。
    // 中文注释: 参见下一行代码含义
    block_seq_num: SeqNum,
    // 执行延迟，用于确定余额窗口。
    // 中文注释: 参见下一行代码含义
    execution_delay: SeqNum,
    // 当前区块的 base fee。
    // 中文注释: 参见下一行代码含义
    base_fee: u64,
    // 共识链的修订信息。
    // 中文注释: 参见下一行代码含义
    chain_revision: CRT,
    // 执行环境的修订信息。
    // 中文注释: 参见下一行代码含义
    execution_chain_revision: MonadExecutionRevision,
    // PhantomData 占位以保存类型信息。
    // 中文注释: 参见下一行代码含义
    _phantom: PhantomData<CRT>,
// 中文注释: 参见下一行代码含义
}

// 判断一笔交易是否可能是“掏空交易”，即远离最近的历史交易。
// 中文注释: 参见下一行代码含义
fn is_possibly_emptying_transaction(
    // 中文注释: 参见下一行代码含义
    block_seq_num_of_curr_txn: SeqNum,
    // 中文注释: 参见下一行代码含义
    balance_state: &AccountBalanceState,
    // 中文注释: 参见下一行代码含义
    execution_delay: SeqNum,
// 中文注释: 参见下一行代码含义
) -> bool {
    // txn T is emptying if there is no "prior txn" i.e. a txn from the same sender sent from block P so that P >= block_number(T) - k + 1.
    // 中文注释: 参见下一行代码含义
    let blocks_since_latest_txn = SeqNum(
        // 中文注释: 参见下一行代码含义
        block_seq_num_of_curr_txn
            // 中文注释: 参见下一行代码含义
            .0
            // 中文注释: 参见下一行代码含义
            .saturating_sub(balance_state.block_seqnum_of_latest_txn.0),
    // 中文注释: 参见下一行代码含义
    );
    // 未被委托且距离最近交易超过执行延迟阈值则视为掏空交易。
    // 中文注释: 参见下一行代码含义
    !balance_state.is_delegated && blocks_since_latest_txn > execution_delay - SeqNum(1)
// 中文注释: 参见下一行代码含义
}

// 将纳秒级时间戳转换为秒，避免超过 u64 上限。
// 中文注释: 参见下一行代码含义
pub fn timestamp_ns_to_secs(timestamp_ns: u128) -> u64 {
    // 中文注释: 参见下一行代码含义
    const NSEC_PER_SEC: u128 = 1_000_000_000;
    // 中文注释: 参见下一行代码含义
    let timestamp_seconds = timestamp_ns / NSEC_PER_SEC;
    // 中文注释: 参见下一行代码含义
    timestamp_seconds.min(u64::MAX.into()) as u64
// 中文注释: 参见下一行代码含义
}

// 中文注释: 参见下一行代码含义
impl<CRT> BlockPolicyBlockValidator<CRT> for EthBlockPolicyBlockValidator<CRT>
// 中文注释: 参见下一行代码含义
where
    // 中文注释: 参见下一行代码含义
    Self: Sized,
    // 中文注释: 参见下一行代码含义
    CRT: ChainRevision,
// 中文注释: 参见下一行代码含义
{
    // 中文注释: 参见下一行代码含义
    type Transaction = Recovered<TxEnvelope>;

    // 中文注释: 参见下一行代码含义
    fn new(
        // 中文注释: 参见下一行代码含义
        block_seq_num: SeqNum,
        // 中文注释: 参见下一行代码含义
        execution_delay: SeqNum,
        // 中文注释: 参见下一行代码含义
        base_fee: u64,
        // 中文注释: 参见下一行代码含义
        chain_revision: &CRT,
        // 中文注释: 参见下一行代码含义
        execution_chain_revision: &MonadExecutionRevision,
    // 中文注释: 参见下一行代码含义
    ) -> Result<Self, BlockPolicyError> {
        // 构造区块策略验证器实例，拷贝链修订信息。
        // 中文注释: 参见下一行代码含义
        Ok(Self {
            // 中文注释: 参见下一行代码含义
            block_seq_num,
            // 中文注释: 参见下一行代码含义
            execution_delay,
            // 中文注释: 参见下一行代码含义
            base_fee,
            // 中文注释: 参见下一行代码含义
            chain_revision: *chain_revision,
            // 中文注释: 参见下一行代码含义
            execution_chain_revision: *execution_chain_revision,
            // 中文注释: 参见下一行代码含义
            _phantom: PhantomData,
        // 中文注释: 参见下一行代码含义
        })
    // 中文注释: 参见下一行代码含义
    }

    // 中文注释: 参见下一行代码含义
    fn try_apply_block_fees(
        // 中文注释: 参见下一行代码含义
        &self,
        // 中文注释: 参见下一行代码含义
        account_balance: &mut AccountBalanceState,
        // 中文注释: 参见下一行代码含义
        block_txn_fees: &TxnFee,
        // 中文注释: 参见下一行代码含义
        eth_address: &Address,
    // 中文注释: 参见下一行代码含义
    ) -> Result<(), BlockPolicyError> {
        // 中文注释: 参见下一行代码含义
        let tfm_enabled = self
            // 中文注释: 参见下一行代码含义
            .execution_chain_revision
            // 中文注释: 参见下一行代码含义
            .execution_chain_params()
            // 中文注释: 参见下一行代码含义
            .tfm_enabled;
        // 中文注释: 参见下一行代码含义
        let max_reserve_balance =
            // 中文注释: 参见下一行代码含义
            Balance::from(self.chain_revision.chain_params().max_reserve_balance);

        // 中文注释: 参见下一行代码含义
        if !tfm_enabled {
            // 中文注释: 参见下一行代码含义
            if account_balance.balance < block_txn_fees.max_txn_cost {
                // 中文注释: 参见下一行代码含义
                trace!(
                    // 中文注释: 参见下一行代码含义
                    seq_num =?self.block_seq_num,
                    // 中文注释: 参见下一行代码含义
                    ?account_balance,
                    // 中文注释: 参见下一行代码含义
                    block_txn_cost =?block_txn_fees.max_txn_cost,
                    // 中文注释: 参见下一行代码含义
                    "TFM disabled. block can not be accepted insufficient balance"
                // 中文注释: 参见下一行代码含义
                );
                // 中文注释: 参见下一行代码含义
                return Err(BlockPolicyError::BlockPolicyBlockValidatorError(
                    // 中文注释: 参见下一行代码含义
                    BlockPolicyBlockValidatorError::InsufficientBalance,
                // 中文注释: 参见下一行代码含义
                ));
            // 中文注释: 参见下一行代码含义
            }

            // 中文注释: 参见下一行代码含义
            let estimated_balance = account_balance
                // 中文注释: 参见下一行代码含义
                .balance
                // 中文注释: 参见下一行代码含义
                .saturating_sub(block_txn_fees.max_txn_cost);
            // 中文注释: 参见下一行代码含义
            account_balance.remaining_reserve_balance = estimated_balance.min(max_reserve_balance);
            // 中文注释: 参见下一行代码含义
            account_balance.balance = estimated_balance;
            // 中文注释: 参见下一行代码含义
            account_balance.block_seqnum_of_latest_txn = self.block_seq_num;

            // 中文注释: 参见下一行代码含义
            trace!(
                // 中文注释: 参见下一行代码含义
                "TFM disabled updated balance: {:?} \
                        // 中文注释: 参见下一行代码含义
                        txn max cost {:?} \
                        // 中文注释: 参见下一行代码含义
                        block seq_num {:?} \
                        // 中文注释: 参见下一行代码含义
                        address: {:?}",
                // 中文注释: 参见下一行代码含义
                account_balance,
                // 中文注释: 参见下一行代码含义
                block_txn_fees.max_txn_cost,
                // 中文注释: 参见下一行代码含义
                self.block_seq_num,
                // 中文注释: 参见下一行代码含义
                eth_address,
            // 中文注释: 参见下一行代码含义
            );
            // 中文注释: 参见下一行代码含义
            return Ok(());
        // 中文注释: 参见下一行代码含义
        }

        // 中文注释: 参见下一行代码含义
        let has_emptying_transaction = is_possibly_emptying_transaction(
            // 中文注释: 参见下一行代码含义
            self.block_seq_num,
            // 中文注释: 参见下一行代码含义
            account_balance,
            // 中文注释: 参见下一行代码含义
            self.execution_delay,
        // 中文注释: 参见下一行代码含义
        );

        // 中文注释: 参见下一行代码含义
        let mut block_gas_cost = block_txn_fees.max_gas_cost;
        // 中文注释: 参见下一行代码含义
        if has_emptying_transaction {
            // 中文注释: 参见下一行代码含义
            if account_balance.balance < block_txn_fees.first_txn_gas {
                // 中文注释: 参见下一行代码含义
                trace!(
                    // 中文注释: 参见下一行代码含义
                    "Block with insufficient balance: {:?} \
                            // 中文注释: 参见下一行代码含义
                            first txn value {:?} \
                            // 中文注释: 参见下一行代码含义
                            first txn gas {:?} \
                            // 中文注释: 参见下一行代码含义
                            block seq_num {:?} \
                            // 中文注释: 参见下一行代码含义
                            address: {:?}",
                    // 中文注释: 参见下一行代码含义
                    account_balance,
                    // 中文注释: 参见下一行代码含义
                    block_txn_fees.first_txn_value,
                    // 中文注释: 参见下一行代码含义
                    block_txn_fees.first_txn_gas,
                    // 中文注释: 参见下一行代码含义
                    self.block_seq_num,
                    // 中文注释: 参见下一行代码含义
                    eth_address,
                // 中文注释: 参见下一行代码含义
                );
                // 中文注释: 参见下一行代码含义
                return Err(BlockPolicyError::BlockPolicyBlockValidatorError(
                    // 中文注释: 参见下一行代码含义
                    BlockPolicyBlockValidatorError::InsufficientBalance,
                // 中文注释: 参见下一行代码含义
                ));
            // 中文注释: 参见下一行代码含义
            }
            // 中文注释: 参见下一行代码含义
            let first_txn_cost = block_txn_fees
                // 中文注释: 参见下一行代码含义
                .first_txn_value
                // 中文注释: 参见下一行代码含义
                .saturating_add(block_txn_fees.first_txn_gas);
            // 中文注释: 参见下一行代码含义
            let estimated_balance = account_balance.balance.saturating_sub(first_txn_cost);

            // 中文注释: 参见下一行代码含义
            account_balance.remaining_reserve_balance = estimated_balance.min(max_reserve_balance);
            // 中文注释: 参见下一行代码含义
            account_balance.balance = estimated_balance;

            // 中文注释: 参见下一行代码含义
            trace!(
                // 中文注释: 参见下一行代码含义
                "Block has emptying txn. updated balance: {:?} \
                        // 中文注释: 参见下一行代码含义
                        first txn value {:?} \
                        // 中文注释: 参见下一行代码含义
                        first txn gas {:?} \
                        // 中文注释: 参见下一行代码含义
                        block seq_num {:?} \
                        // 中文注释: 参见下一行代码含义
                        address: {:?}",
                // 中文注释: 参见下一行代码含义
                account_balance,
                // 中文注释: 参见下一行代码含义
                block_txn_fees.first_txn_value,
                // 中文注释: 参见下一行代码含义
                block_txn_fees.first_txn_gas,
                // 中文注释: 参见下一行代码含义
                self.block_seq_num,
                // 中文注释: 参见下一行代码含义
                eth_address,
            // 中文注释: 参见下一行代码含义
            );
        // 中文注释: 参见下一行代码含义
        } else {
            // 中文注释: 参见下一行代码含义
            block_gas_cost = block_txn_fees
                // 中文注释: 参见下一行代码含义
                .max_gas_cost
                // 中文注释: 参见下一行代码含义
                .saturating_add(block_txn_fees.first_txn_gas);
        // 中文注释: 参见下一行代码含义
        }

        // 中文注释: 参见下一行代码含义
        if account_balance.remaining_reserve_balance < block_gas_cost {
            // 中文注释: 参见下一行代码含义
            trace!(
                // 中文注释: 参见下一行代码含义
                "Block with insufficient reserve balance: {:?} \
                            // 中文注释: 参见下一行代码含义
                            max gas cost {:?} \
                            // 中文注释: 参见下一行代码含义
                            block seq_num {:?} \
                            // 中文注释: 参见下一行代码含义
                            address: {:?}",
                // 中文注释: 参见下一行代码含义
                account_balance,
                // 中文注释: 参见下一行代码含义
                block_gas_cost,
                // 中文注释: 参见下一行代码含义
                self.block_seq_num,
                // 中文注释: 参见下一行代码含义
                eth_address,
            // 中文注释: 参见下一行代码含义
            );
            // 中文注释: 参见下一行代码含义
            return Err(BlockPolicyError::BlockPolicyBlockValidatorError(
                // 中文注释: 参见下一行代码含义
                BlockPolicyBlockValidatorError::InsufficientReserveBalance,
            // 中文注释: 参见下一行代码含义
            ));
        // 中文注释: 参见下一行代码含义
        }
        // 中文注释: 参见下一行代码含义
        account_balance.remaining_reserve_balance = account_balance
            // 中文注释: 参见下一行代码含义
            .remaining_reserve_balance
            // 中文注释: 参见下一行代码含义
            .saturating_sub(block_gas_cost);
        // 中文注释: 参见下一行代码含义
        account_balance.block_seqnum_of_latest_txn = self.block_seq_num;
        // 中文注释: 参见下一行代码含义
        account_balance.is_delegated |= block_txn_fees.is_delegated;

        // 中文注释: 参见下一行代码含义
        trace!(
            // 中文注释: 参见下一行代码含义
            ?account_balance,
            // 中文注释: 参见下一行代码含义
            ?self.block_seq_num,
            // 中文注释: 参见下一行代码含义
            ?eth_address,
            // 中文注释: 参见下一行代码含义
            "try_apply_block_fees updated balance state",
        // 中文注释: 参见下一行代码含义
        );
        // 中文注释: 参见下一行代码含义
        Ok(())
    // 中文注释: 参见下一行代码含义
    }

    // 中文注释: 参见下一行代码含义
    fn try_add_transaction(
        // 中文注释: 参见下一行代码含义
        &self,
        // 中文注释: 参见下一行代码含义
        account_balances: &mut BTreeMap<&Address, AccountBalanceState>,
        // 中文注释: 参见下一行代码含义
        txn: &Self::Transaction,
    // 中文注释: 参见下一行代码含义
    ) -> Result<(), BlockPolicyError> {
        // 中文注释: 参见下一行代码含义
        let eth_address = txn.signer();

        // 中文注释: 参见下一行代码含义
        let maybe_account_balance = account_balances.get_mut(&eth_address);

        // 中文注释: 参见下一行代码含义
        let Some(account_balance) = maybe_account_balance else {
            // 中文注释: 参见下一行代码含义
            warn!(
                // 中文注释: 参见下一行代码含义
                seq_num =?self.block_seq_num,
                // 中文注释: 参见下一行代码含义
                ?eth_address,
                // 中文注释: 参见下一行代码含义
                "account balance have not been populated"
            // 中文注释: 参见下一行代码含义
            );
            // 中文注释: 参见下一行代码含义
            return Err(BlockPolicyError::BlockPolicyBlockValidatorError(
                // 中文注释: 参见下一行代码含义
                BlockPolicyBlockValidatorError::AccountBalanceMissing,
            // 中文注释: 参见下一行代码含义
            ));
        // 中文注释: 参见下一行代码含义
        };

        // 中文注释: 参见下一行代码含义
        if !self
            // 中文注释: 参见下一行代码含义
            .execution_chain_revision
            // 中文注释: 参见下一行代码含义
            .execution_chain_params()
            // 中文注释: 参见下一行代码含义
            .tfm_enabled
        // 中文注释: 参见下一行代码含义
        {
            // 中文注释: 参见下一行代码含义
            let txn_cost = pre_tfm_compute_max_txn_cost(txn);
            // 中文注释: 参见下一行代码含义
            if account_balance.balance < txn_cost {
                // 中文注释: 参见下一行代码含义
                trace!(
                    // 中文注释: 参见下一行代码含义
                    seq_num =?self.block_seq_num,
                    // 中文注释: 参见下一行代码含义
                    ?account_balance,
                    // 中文注释: 参见下一行代码含义
                    ?txn_cost,
                    // 中文注释: 参见下一行代码含义
                    ?txn,
                    // 中文注释: 参见下一行代码含义
                    "TFM disabled. txn can not be accepted insufficient balance"
                // 中文注释: 参见下一行代码含义
                );
                // 中文注释: 参见下一行代码含义
                return Err(BlockPolicyError::BlockPolicyBlockValidatorError(
                    // 中文注释: 参见下一行代码含义
                    BlockPolicyBlockValidatorError::InsufficientBalance,
                // 中文注释: 参见下一行代码含义
                ));
            // 中文注释: 参见下一行代码含义
            }

            // 中文注释: 参见下一行代码含义
            let estimated_balance = account_balance.balance.saturating_sub(txn_cost);
            // 中文注释: 参见下一行代码含义
            account_balance.remaining_reserve_balance =
                // 中文注释: 参见下一行代码含义
                estimated_balance.min(account_balance.max_reserve_balance);
            // 中文注释: 参见下一行代码含义
            account_balance.balance = estimated_balance;
            // 中文注释: 参见下一行代码含义
            account_balance.block_seqnum_of_latest_txn = self.block_seq_num;

            // 中文注释: 参见下一行代码含义
            trace!(
                // 中文注释: 参见下一行代码含义
                "TFM disabled. updated balance: {:?} \
                        // 中文注释: 参见下一行代码含义
                        txn cost {:?} \
                        // 中文注释: 参见下一行代码含义
                        block seq_num {:?} \
                        // 中文注释: 参见下一行代码含义
                        address: {:?}",
                // 中文注释: 参见下一行代码含义
                account_balance,
                // 中文注释: 参见下一行代码含义
                txn_cost,
                // 中文注释: 参见下一行代码含义
                self.block_seq_num,
                // 中文注释: 参见下一行代码含义
                eth_address,
            // 中文注释: 参见下一行代码含义
            );
            // 中文注释: 参见下一行代码含义
            return Ok(());
        // 中文注释: 参见下一行代码含义
        }

        // 中文注释: 参见下一行代码含义
        let is_emptying_transaction = is_possibly_emptying_transaction(
            // 中文注释: 参见下一行代码含义
            self.block_seq_num,
            // 中文注释: 参见下一行代码含义
            account_balance,
            // 中文注释: 参见下一行代码含义
            self.execution_delay,
        // 中文注释: 参见下一行代码含义
        );

        // if an account for txn T is not delegated and has no prior txns, then T can charge into reserve.
        // 中文注释: 参见下一行代码含义
        if is_emptying_transaction {
            // 中文注释: 参见下一行代码含义
            let txn_max_gas = compute_txn_max_gas_cost(txn, self.base_fee);
            // 中文注释: 参见下一行代码含义
            if account_balance.balance < txn_max_gas {
                // 中文注释: 参见下一行代码含义
                trace!(
                    // 中文注释: 参见下一行代码含义
                    seq_num =?self.block_seq_num,
                    // 中文注释: 参见下一行代码含义
                    ?account_balance,
                    // 中文注释: 参见下一行代码含义
                    ?txn_max_gas,
                    // 中文注释: 参见下一行代码含义
                    ?txn,
                    // 中文注释: 参见下一行代码含义
                    ?is_emptying_transaction,
                    // 中文注释: 参见下一行代码含义
                    "Emptying txn can not be accepted insufficient reserve balance"
                // 中文注释: 参见下一行代码含义
                );
                // 中文注释: 参见下一行代码含义
                return Err(BlockPolicyError::BlockPolicyBlockValidatorError(
                    // 中文注释: 参见下一行代码含义
                    BlockPolicyBlockValidatorError::InsufficientBalance,
                // 中文注释: 参见下一行代码含义
                ));
            // 中文注释: 参见下一行代码含义
            }

            // 中文注释: 参见下一行代码含义
            let txn_max_cost = compute_txn_max_value(txn, self.base_fee);
            // 中文注释: 参见下一行代码含义
            let estimated_balance = account_balance.balance.saturating_sub(txn_max_cost);
            // 中文注释: 参见下一行代码含义
            let reserve_balance = account_balance.max_reserve_balance.min(estimated_balance);

            // 中文注释: 参见下一行代码含义
            trace!(
                // 中文注释: 参见下一行代码含义
                "New emptying txn. balance: {:?} \
                    // 中文注释: 参见下一行代码含义
                    txn_max_cost {:?} \
                    // 中文注释: 参见下一行代码含义
                    txn_max_gas {:?} \
                    // 中文注释: 参见下一行代码含义
                    estimated_balance {:?} \
                    // 中文注释: 参见下一行代码含义
                    new reserve balance {:?} \
                    // 中文注释: 参见下一行代码含义
                    block seq_num {:?} \
                    // 中文注释: 参见下一行代码含义
                    address: {:?}",
                // 中文注释: 参见下一行代码含义
                account_balance,
                // 中文注释: 参见下一行代码含义
                txn_max_cost,
                // 中文注释: 参见下一行代码含义
                txn_max_gas,
                // 中文注释: 参见下一行代码含义
                estimated_balance,
                // 中文注释: 参见下一行代码含义
                reserve_balance,
                // 中文注释: 参见下一行代码含义
                self.block_seq_num,
                // 中文注释: 参见下一行代码含义
                eth_address,
            // 中文注释: 参见下一行代码含义
            );
            // 中文注释: 参见下一行代码含义
            account_balance.balance = estimated_balance;
            // 中文注释: 参见下一行代码含义
            account_balance.remaining_reserve_balance = reserve_balance;
            // 中文注释: 参见下一行代码含义
            account_balance.block_seqnum_of_latest_txn = self.block_seq_num;
        // 中文注释: 参见下一行代码含义
        } else {
            // 中文注释: 参见下一行代码含义
            let txn_max_gas = compute_txn_max_gas_cost(txn, self.base_fee);
            // 中文注释: 参见下一行代码含义
            if account_balance.remaining_reserve_balance < txn_max_gas {
                // 中文注释: 参见下一行代码含义
                trace!(
                    // 中文注释: 参见下一行代码含义
                    seq_num =?self.block_seq_num,
                    // 中文注释: 参见下一行代码含义
                    ?account_balance,
                    // 中文注释: 参见下一行代码含义
                    ?txn_max_gas,
                    // 中文注释: 参见下一行代码含义
                    ?txn,
                    // 中文注释: 参见下一行代码含义
                    ?is_emptying_transaction,
                    // 中文注释: 参见下一行代码含义
                    "Non-emptying txn can not be accepted insufficient reserve balance"
                // 中文注释: 参见下一行代码含义
                );
                // 中文注释: 参见下一行代码含义
                return Err(BlockPolicyError::BlockPolicyBlockValidatorError(
                    // 中文注释: 参见下一行代码含义
                    BlockPolicyBlockValidatorError::InsufficientReserveBalance,
                // 中文注释: 参见下一行代码含义
                ));
            // 中文注释: 参见下一行代码含义
            }
            // 中文注释: 参见下一行代码含义
            let reserve_balance = account_balance
                // 中文注释: 参见下一行代码含义
                .remaining_reserve_balance
                // 中文注释: 参见下一行代码含义
                .saturating_sub(txn_max_gas);

            // 中文注释: 参见下一行代码含义
            account_balance.remaining_reserve_balance = reserve_balance;
            // 中文注释: 参见下一行代码含义
            account_balance.block_seqnum_of_latest_txn = self.block_seq_num;
        // 中文注释: 参见下一行代码含义
        }

        // 中文注释: 参见下一行代码含义
        Ok(())
    // 中文注释: 参见下一行代码含义
    }
// 中文注释: 参见下一行代码含义
}

/// A block policy for ethereum payloads
// 中文注释: 参见下一行代码含义
#[derive(Debug)]
// 中文注释: 参见下一行代码含义
pub struct EthBlockPolicy<ST, SCT, CCT, CRT>
// 中文注释: 参见下一行代码含义
where
    // 中文注释: 参见下一行代码含义
    ST: CertificateSignatureRecoverable,
    // 中文注释: 参见下一行代码含义
    SCT: SignatureCollection<NodeIdPubKey = CertificateSignaturePubKey<ST>>,
// 中文注释: 参见下一行代码含义
{
    /// SeqNum of last committed block
    // 中文注释: 参见下一行代码含义
    last_commit: SeqNum,

    // last execution-delay committed blocks
    // 中文注释: 参见下一行代码含义
    committed_cache: CommittedBlkBuffer<ST, SCT, CCT, CRT>,

    // 中文注释: 参见下一行代码含义
    execution_delay: SeqNum,
// 中文注释: 参见下一行代码含义
}

// 中文注释: 参见下一行代码含义
impl<ST, SCT, CCT, CRT> EthBlockPolicy<ST, SCT, CCT, CRT>
// 中文注释: 参见下一行代码含义
where
    // 中文注释: 参见下一行代码含义
    ST: CertificateSignatureRecoverable,
    // 中文注释: 参见下一行代码含义
    SCT: SignatureCollection<NodeIdPubKey = CertificateSignaturePubKey<ST>>,
    // 中文注释: 参见下一行代码含义
    CCT: ChainConfig<CRT>,
    // 中文注释: 参见下一行代码含义
    CRT: ChainRevision,
// 中文注释: 参见下一行代码含义
{
    // 中文注释: 参见下一行代码含义
    pub fn new(
        // 中文注释: 参见下一行代码含义
        last_commit: SeqNum, // TODO deprecate
        // 中文注释: 参见下一行代码含义
        execution_delay: u64,
    // 中文注释: 参见下一行代码含义
    ) -> Self {
        // 中文注释: 参见下一行代码含义
        let cache_max_size = execution_delay.saturating_mul(2);
        // 中文注释: 参见下一行代码含义
        Self {
            // Needs to be at least 2 * execution_delay to detect emptying transactions
            // 中文注释: 参见下一行代码含义
            committed_cache: CommittedBlkBuffer::new((cache_max_size) as usize),
            // 中文注释: 参见下一行代码含义
            last_commit,
            // 中文注释: 参见下一行代码含义
            execution_delay: SeqNum(execution_delay),
        // 中文注释: 参见下一行代码含义
        }
    // 中文注释: 参见下一行代码含义
    }

    /// returns account nonces at the start of the provided consensus block
    // 中文注释: 参见下一行代码含义
    pub fn get_account_base_nonces<'a>(
        // 中文注释: 参见下一行代码含义
        &self,
        // 中文注释: 参见下一行代码含义
        consensus_block_seq_num: SeqNum,
        // 中文注释: 参见下一行代码含义
        state_backend: &impl StateBackend<ST, SCT>,
        // 中文注释: 参见下一行代码含义
        extending_blocks: &Vec<&EthValidatedBlock<ST, SCT>>,
        // 中文注释: 参见下一行代码含义
        addresses: impl Iterator<Item = &'a Address>,
    // 中文注释: 参见下一行代码含义
    ) -> Result<BTreeMap<&'a Address, Nonce>, StateBackendError> {
        // Layers of access
        // 1. extending_blocks: coherent blocks in the blocks tree
        // 2. committed_block_nonces: always buffers the nonce of last `delay`
        //    committed blocks
        // 3. LRU cache of triedb nonces
        // 4. triedb query

        // 中文注释: 参见下一行代码含义
        let addresses = addresses.unique().collect::<HashSet<&'a Address>>();

        // 中文注释: 参见下一行代码含义
        let base_seq_num = consensus_block_seq_num.max(self.execution_delay) - self.execution_delay;

        // 中文注释: 参见下一行代码含义
        let mut cached_nonce_usages = NonceUsageMap::default();

        // 中文注释: 参见下一行代码含义
        for nonce_usages in self
            // 中文注释: 参见下一行代码含义
            .committed_cache
            // 中文注释: 参见下一行代码含义
            .blocks
            // 中文注释: 参见下一行代码含义
            .iter()
            // 中文注释: 参见下一行代码含义
            .map(|(seq_num, block)| (*seq_num, &block.nonce_usages))
            // 中文注释: 参见下一行代码含义
            .chain(
                // 中文注释: 参见下一行代码含义
                extending_blocks
                    // 中文注释: 参见下一行代码含义
                    .iter()
                    // 中文注释: 参见下一行代码含义
                    .map(|block| (block.get_seq_num(), &block.nonce_usages)),
            // 中文注释: 参见下一行代码含义
            )
            // 中文注释: 参见下一行代码含义
            .filter(|(seq_num, _)| *seq_num > base_seq_num)
            // 中文注释: 参见下一行代码含义
            .rev()
            // 中文注释: 参见下一行代码含义
            .map(|(_, nonce_usages)| {
                // 中文注释: 参见下一行代码含义
                nonce_usages
                    // 中文注释: 参见下一行代码含义
                    .map
                    // 中文注释: 参见下一行代码含义
                    .iter()
                    // 中文注释: 参见下一行代码含义
                    .filter(|(address, _)| addresses.contains(address))
            // 中文注释: 参见下一行代码含义
            })
        // 中文注释: 参见下一行代码含义
        {
            // 中文注释: 参见下一行代码含义
            cached_nonce_usages.merge_with_previous_block(nonce_usages);
        // 中文注释: 参见下一行代码含义
        }

        // 中文注释: 参见下一行代码含义
        let mut account_nonces = BTreeMap::default();
        // 中文注释: 参见下一行代码含义
        let mut cache_misses = Vec::new();

        // 中文注释: 参见下一行代码含义
        for address in addresses {
            // 中文注释: 参见下一行代码含义
            match cached_nonce_usages.get(address) {
                // 中文注释: 参见下一行代码含义
                Some(NonceUsage::Known(nonce)) => {
                    // 中文注释: 参见下一行代码含义
                    account_nonces.insert(address, *nonce + 1);
                // 中文注释: 参见下一行代码含义
                }
                // 中文注释: 参见下一行代码含义
                Some(NonceUsage::Possible(possible)) => {
                    // 中文注释: 参见下一行代码含义
                    cache_misses.push((address, Some(possible)));
                // 中文注释: 参见下一行代码含义
                }
                // 中文注释: 参见下一行代码含义
                None => {
                    // 中文注释: 参见下一行代码含义
                    cache_misses.push((address, None));
                // 中文注释: 参见下一行代码含义
                }
            // 中文注释: 参见下一行代码含义
            }
        // 中文注释: 参见下一行代码含义
        }

        // the cached account nonce must overlap with latest triedb, i.e.
        // account_nonces must keep nonces for last delay blocks in cache
        // the cache should keep track of block number for the nonce state
        // when purging, we never purge nonces newer than last_commit - delay

        // 中文注释: 参见下一行代码含义
        let cache_miss_statuses = self.get_account_statuses(
            // 中文注释: 参见下一行代码含义
            state_backend,
            // 中文注释: 参见下一行代码含义
            &Some(extending_blocks),
            // 中文注释: 参见下一行代码含义
            cache_misses.iter().map(|(address, _)| *address),
            // 中文注释: 参见下一行代码含义
            &base_seq_num,
        // 中文注释: 参见下一行代码含义
        )?;

        // 中文注释: 参见下一行代码含义
        account_nonces.extend(cache_misses.into_iter().zip_eq(cache_miss_statuses).map(
            // 中文注释: 参见下一行代码含义
            |((address, possible_nonces), status)| {
                // 中文注释: 参见下一行代码含义
                let nonce = status.map_or(0, |status| status.nonce);

                // 中文注释: 参见下一行代码含义
                (
                    // 中文注释: 参见下一行代码含义
                    address,
                    // 中文注释: 参见下一行代码含义
                    possible_nonces.map_or(nonce, |possible_nonces| {
                        // 中文注释: 参见下一行代码含义
                        NonceUsage::apply_possible_nonces_to_account_nonce(nonce, possible_nonces)
                    // 中文注释: 参见下一行代码含义
                    }),
                // 中文注释: 参见下一行代码含义
                )
            // 中文注释: 参见下一行代码含义
            },
        // 中文注释: 参见下一行代码含义
        ));

        // 中文注释: 参见下一行代码含义
        Ok(account_nonces)
    // 中文注释: 参见下一行代码含义
    }

    // 中文注释: 参见下一行代码含义
    pub fn get_last_commit(&self) -> SeqNum {
        // 中文注释: 参见下一行代码含义
        self.last_commit
    // 中文注释: 参见下一行代码含义
    }

    // 中文注释: 参见下一行代码含义
    pub fn get_last_commit_epoch(&self) -> Epoch {
        // 中文注释: 参见下一行代码含义
        if self.last_commit == GENESIS_SEQ_NUM {
            // 中文注释: 参见下一行代码含义
            Epoch(1)
        // 中文注释: 参见下一行代码含义
        } else {
            // 中文注释: 参见下一行代码含义
            self.committed_cache
                // 中文注释: 参见下一行代码含义
                .get_epoch(self.last_commit)
                // 中文注释: 参见下一行代码含义
                .expect("last committed block in committed cache")
        // 中文注释: 参见下一行代码含义
        }
    // 中文注释: 参见下一行代码含义
    }

    // 中文注释: 参见下一行代码含义
    fn get_block_index(
        // 中文注释: 参见下一行代码含义
        &self,
        // 中文注释: 参见下一行代码含义
        extending_blocks: &Option<&Vec<&EthValidatedBlock<ST, SCT>>>,
        // 中文注释: 参见下一行代码含义
        base_seq_num: &SeqNum,
    // 中文注释: 参见下一行代码含义
    ) -> Result<BlockLookupIndex, StateBackendError> {
        // 中文注释: 参见下一行代码含义
        if base_seq_num <= &self.last_commit {
            // 中文注释: 参见下一行代码含义
            if base_seq_num == &GENESIS_SEQ_NUM {
                // 中文注释: 参见下一行代码含义
                Ok(BlockLookupIndex {
                    // 中文注释: 参见下一行代码含义
                    block_id: GENESIS_BLOCK_ID,
                    // 中文注释: 参见下一行代码含义
                    seq_num: GENESIS_SEQ_NUM,
                    // 中文注释: 参见下一行代码含义
                    round: GENESIS_ROUND,
                    // 中文注释: 参见下一行代码含义
                    is_finalized: true,
                // 中文注释: 参见下一行代码含义
                })
            // 中文注释: 参见下一行代码含义
            } else {
                // 中文注释: 参见下一行代码含义
                let committed_block = &self
                    // 中文注释: 参见下一行代码含义
                    .committed_cache
                    // 中文注释: 参见下一行代码含义
                    .blocks
                    // 中文注释: 参见下一行代码含义
                    .get(base_seq_num)
                    // 中文注释: 参见下一行代码含义
                    .unwrap_or_else(|| panic!("queried recently committed block that doesn't exist, base_seq_num={:?}, last_commit={:?}", base_seq_num, self.last_commit));
                // 中文注释: 参见下一行代码含义
                Ok(BlockLookupIndex {
                    // 中文注释: 参见下一行代码含义
                    block_id: committed_block.block_id,
                    // 中文注释: 参见下一行代码含义
                    seq_num: *base_seq_num,
                    // 中文注释: 参见下一行代码含义
                    round: committed_block.round,
                    // 中文注释: 参见下一行代码含义
                    is_finalized: true,
                // 中文注释: 参见下一行代码含义
                })
            // 中文注释: 参见下一行代码含义
            }
        // 中文注释: 参见下一行代码含义
        } else if let Some(extending_blocks) = extending_blocks {
            // 中文注释: 参见下一行代码含义
            let proposed_block = extending_blocks
                // 中文注释: 参见下一行代码含义
                .iter()
                // 中文注释: 参见下一行代码含义
                .find(|block| &block.get_seq_num() == base_seq_num)
                // 中文注释: 参见下一行代码含义
                .expect("extending block doesn't exist");
            // 中文注释: 参见下一行代码含义
            Ok(BlockLookupIndex {
                // 中文注释: 参见下一行代码含义
                block_id: proposed_block.get_id(),
                // 中文注释: 参见下一行代码含义
                seq_num: *base_seq_num,
                // 中文注释: 参见下一行代码含义
                round: proposed_block.get_block_round(),
                // 中文注释: 参见下一行代码含义
                is_finalized: false,
            // 中文注释: 参见下一行代码含义
            })
        // 中文注释: 参见下一行代码含义
        } else {
            // 中文注释: 参见下一行代码含义
            Err(StateBackendError::NotAvailableYet)
        // 中文注释: 参见下一行代码含义
        }
    // 中文注释: 参见下一行代码含义
    }

    // 中文注释: 参见下一行代码含义
    fn get_account_statuses<'a>(
        // 中文注释: 参见下一行代码含义
        &self,
        // 中文注释: 参见下一行代码含义
        state_backend: &impl StateBackend<ST, SCT>,
        // 中文注释: 参见下一行代码含义
        extending_blocks: &Option<&Vec<&EthValidatedBlock<ST, SCT>>>,
        // 中文注释: 参见下一行代码含义
        addresses: impl Iterator<Item = &'a Address>,
        // 中文注释: 参见下一行代码含义
        base_seq_num: &SeqNum,
    // 中文注释: 参见下一行代码含义
    ) -> Result<Vec<Option<EthAccount>>, StateBackendError> {
        // 中文注释: 参见下一行代码含义
        let block_index = self.get_block_index(extending_blocks, base_seq_num)?;
        // 中文注释: 参见下一行代码含义
        state_backend.get_account_statuses(
            // 中文注释: 参见下一行代码含义
            &block_index.block_id,
            // 中文注释: 参见下一行代码含义
            base_seq_num,
            // 中文注释: 参见下一行代码含义
            block_index.is_finalized,
            // 中文注释: 参见下一行代码含义
            addresses,
        // 中文注释: 参见下一行代码含义
        )
    // 中文注释: 参见下一行代码含义
    }

    // Computes account balance available for the account
    // 中文注释: 参见下一行代码含义
    pub fn compute_account_base_balances<'a>(
        // 中文注释: 参见下一行代码含义
        &self,
        // 中文注释: 参见下一行代码含义
        consensus_block_seq_num: SeqNum,
        // 中文注释: 参见下一行代码含义
        state_backend: &impl StateBackend<ST, SCT>,
        // 中文注释: 参见下一行代码含义
        chain_config: &CCT,
        // 中文注释: 参见下一行代码含义
        extending_blocks: Option<&Vec<&EthValidatedBlock<ST, SCT>>>,
        // 中文注释: 参见下一行代码含义
        addresses: impl Iterator<Item = &'a Address>,
    // 中文注释: 参见下一行代码含义
    ) -> Result<BTreeMap<&'a Address, AccountBalanceState>, BlockPolicyError>
    // 中文注释: 参见下一行代码含义
    where
        // 中文注释: 参见下一行代码含义
        SCT: SignatureCollection,
    // 中文注释: 参见下一行代码含义
    {
        // calculation correct only if GENESIS_SEQ_NUM == 0
        // 中文注释: 参见下一行代码含义
        assert_eq!(GENESIS_SEQ_NUM, SeqNum(0));
        // 中文注释: 参见下一行代码含义
        let base_seq_num = consensus_block_seq_num.max(self.execution_delay) - self.execution_delay;

        // 中文注释: 参见下一行代码含义
        let block_index = self.get_block_index(&extending_blocks, &base_seq_num)?;
        // 中文注释: 参见下一行代码含义
        let base_max_reserve_balance = Balance::from(
            // 中文注释: 参见下一行代码含义
            chain_config
                // 中文注释: 参见下一行代码含义
                .get_chain_revision(block_index.round)
                // 中文注释: 参见下一行代码含义
                .chain_params()
                // 中文注释: 参见下一行代码含义
                .max_reserve_balance,
        // 中文注释: 参见下一行代码含义
        );

        // 中文注释: 参见下一行代码含义
        let addresses = addresses.unique().collect_vec();
        // 中文注释: 参见下一行代码含义
        let account_balances = self
            // 中文注释: 参见下一行代码含义
            .get_account_statuses(
                // 中文注释: 参见下一行代码含义
                state_backend,
                // 中文注释: 参见下一行代码含义
                &extending_blocks,
                // 中文注释: 参见下一行代码含义
                addresses.iter().copied(),
                // 中文注释: 参见下一行代码含义
                &base_seq_num,
            // 中文注释: 参见下一行代码含义
            )?
            // 中文注释: 参见下一行代码含义
            .into_iter()
            // 中文注释: 参见下一行代码含义
            .map(|maybe_status| {
                // 中文注释: 参见下一行代码含义
                maybe_status.map_or(
                    // 中文注释: 参见下一行代码含义
                    AccountBalanceState::new(base_max_reserve_balance),
                    // 中文注释: 参见下一行代码含义
                    |status| {
                        // 中文注释: 参见下一行代码含义
                        AccountBalanceState {
                            // 中文注释: 参见下一行代码含义
                            balance: status.balance,
                            // 中文注释: 参见下一行代码含义
                            remaining_reserve_balance: status.balance.min(base_max_reserve_balance),
                            // 中文注释: 参见下一行代码含义
                            max_reserve_balance: base_max_reserve_balance,
                            // 中文注释: 参见下一行代码含义
                            block_seqnum_of_latest_txn: base_seq_num, // most pessimistic assumption
                            // 中文注释: 参见下一行代码含义
                            is_delegated: status.is_delegated,
                        // 中文注释: 参见下一行代码含义
                        }
                    // 中文注释: 参见下一行代码含义
                    },
                // 中文注释: 参见下一行代码含义
                )
            // 中文注释: 参见下一行代码含义
            })
            // 中文注释: 参见下一行代码含义
            .collect_vec();

        // 中文注释: 参见下一行代码含义
        let account_balances: Result<BTreeMap<&'a Address, AccountBalanceState>, BlockPolicyError> =
            // 中文注释: 参见下一行代码含义
            addresses
                // 中文注释: 参见下一行代码含义
                .into_iter()
                // 中文注释: 参见下一行代码含义
                .zip_eq(account_balances)
                // 中文注释: 参见下一行代码含义
                .map(|(address, mut balance_state)| {
                    // N - k + 1
                    // 中文注释: 参见下一行代码含义
                    let reserve_balance_check_start = base_seq_num + SeqNum(1);
                    // N - 2k + 2
                    // 中文注释: 参见下一行代码含义
                    let mut emptying_txn_check_start = (reserve_balance_check_start + SeqNum(1))
                        // 中文注释: 参见下一行代码含义
                        .max(self.execution_delay)
                        // 中文注释: 参见下一行代码含义
                        - self.execution_delay;

                    // 中文注释: 参见下一行代码含义
                    if emptying_txn_check_start == GENESIS_SEQ_NUM {
                        // 中文注释: 参见下一行代码含义
                        emptying_txn_check_start += SeqNum(1);
                    // 中文注释: 参见下一行代码含义
                    }

                    // N - 2k + 2 (inclusive) to N - k + 1 (non inclusive)
                    // 中文注释: 参见下一行代码含义
                    let emptying_txn_check_block_range =
                        // 中文注释: 参见下一行代码含义
                        emptying_txn_check_start..reserve_balance_check_start;
                    // N - k + 1 (inclusive) to N (non inclusive)
                    // 中文注释: 参见下一行代码含义
                    let reserve_balance_check_block_range = reserve_balance_check_start..;

                    // 中文注释: 参见下一行代码含义
                    if emptying_txn_check_start > GENESIS_SEQ_NUM {
                        // 中文注释: 参见下一行代码含义
                        balance_state.block_seqnum_of_latest_txn =
                            // 中文注释: 参见下一行代码含义
                            emptying_txn_check_start - SeqNum(1);
                    // 中文注释: 参见下一行代码含义
                    }

                    // check for emptying txs and reserve balance in committed blocks
                    // 中文注释: 参见下一行代码含义
                    let mut next_validate = self.committed_cache.update_account_balance(
                        // 中文注释: 参见下一行代码含义
                        &mut balance_state,
                        // 中文注释: 参见下一行代码含义
                        address,
                        // 中文注释: 参见下一行代码含义
                        self.execution_delay,
                        // 中文注释: 参见下一行代码含义
                        emptying_txn_check_block_range,
                        // 中文注释: 参见下一行代码含义
                        reserve_balance_check_block_range,
                        // 中文注释: 参见下一行代码含义
                        chain_config,
                    // 中文注释: 参见下一行代码含义
                    )?;

                    // check for emptying txs and reserve balance in extending blocks
                    // 中文注释: 参见下一行代码含义
                    if let Some(blocks) = extending_blocks {
                        // handle the case where base_seq_num is a pending block
                        // 中文注释: 参见下一行代码含义
                        let next_blocks = blocks
                            // 中文注释: 参见下一行代码含义
                            .iter()
                            // 中文注释: 参见下一行代码含义
                            .skip_while(move |block| block.get_seq_num() < next_validate);

                        // 中文注释: 参见下一行代码含义
                        for extending_block in next_blocks {
                            // 中文注释: 参见下一行代码含义
                            assert_eq!(next_validate, extending_block.get_seq_num());

                            // 中文注释: 参见下一行代码含义
                            if let Some(txn_fee) = extending_block.txn_fees.get(address) {
                                // if still within check emptying range, update latest tx seq num
                                // otherwise check for reserve balance
                                // 中文注释: 参见下一行代码含义
                                if next_validate < reserve_balance_check_start {
                                    // 中文注释: 参见下一行代码含义
                                    if balance_state.block_seqnum_of_latest_txn < next_validate {
                                        // 中文注释: 参见下一行代码含义
                                        balance_state.block_seqnum_of_latest_txn =
                                            // 中文注释: 参见下一行代码含义
                                            extending_block.get_seq_num();
                                    // 中文注释: 参见下一行代码含义
                                    }
                                // 中文注释: 参见下一行代码含义
                                } else {
                                    // 中文注释: 参见下一行代码含义
                                    let validator = EthBlockPolicyBlockValidator::new(
                                        // 中文注释: 参见下一行代码含义
                                        extending_block.get_seq_num(),
                                        // 中文注释: 参见下一行代码含义
                                        self.execution_delay,
                                        // 中文注释: 参见下一行代码含义
                                        extending_block
                                            // 中文注释: 参见下一行代码含义
                                            .get_base_fee()
                                            // 中文注释: 参见下一行代码含义
                                            .unwrap_or(monad_tfm::base_fee::PRE_TFM_BASE_FEE),
                                        // 中文注释: 参见下一行代码含义
                                        &chain_config
                                            // 中文注释: 参见下一行代码含义
                                            .get_chain_revision(extending_block.get_block_round()),
                                        // 中文注释: 参见下一行代码含义
                                        &chain_config.get_execution_chain_revision(
                                            // 中文注释: 参见下一行代码含义
                                            timestamp_ns_to_secs(extending_block.get_timestamp()),
                                        // 中文注释: 参见下一行代码含义
                                        ),
                                    // 中文注释: 参见下一行代码含义
                                    )?;

                                    // 中文注释: 参见下一行代码含义
                                    validator.try_apply_block_fees(
                                        // 中文注释: 参见下一行代码含义
                                        &mut balance_state,
                                        // 中文注释: 参见下一行代码含义
                                        txn_fee,
                                        // 中文注释: 参见下一行代码含义
                                        address,
                                    // 中文注释: 参见下一行代码含义
                                    )?;
                                // 中文注释: 参见下一行代码含义
                                }
                            // 中文注释: 参见下一行代码含义
                            }
                            // 中文注释: 参见下一行代码含义
                            next_validate += SeqNum(1);
                        // 中文注释: 参见下一行代码含义
                        }
                    // 中文注释: 参见下一行代码含义
                    }

                    // 中文注释: 参见下一行代码含义
                    Ok((address, balance_state))
                // 中文注释: 参见下一行代码含义
                })
                // 中文注释: 参见下一行代码含义
                .collect();
        // 中文注释: 参见下一行代码含义
        account_balances
    // 中文注释: 参见下一行代码含义
    }

    /// return value:
    /// (parent_block_round, parent_base_fee, parent_trend, parent_moment, parent_gas_usage)
    // 中文注释: 参见下一行代码含义
    fn get_parent_base_fee_fields<B>(&self, extending_blocks: &[B]) -> (Round, u64, u64, u64, u64)
    // 中文注释: 参见下一行代码含义
    where
        // 中文注释: 参见下一行代码含义
        B: AsRef<EthValidatedBlock<ST, SCT>>,
    // 中文注释: 参见下一行代码含义
    {
        // parent block is last block in extending_blocks or last_committed
        // block if there's no extending branch
        // 中文注释: 参见下一行代码含义
        let (
            // 中文注释: 参见下一行代码含义
            parent_block_round,
            // 中文注释: 参见下一行代码含义
            maybe_parent_base_fee,
            // 中文注释: 参见下一行代码含义
            maybe_parent_trend,
            // 中文注释: 参见下一行代码含义
            maybe_parent_moment,
            // 中文注释: 参见下一行代码含义
            parent_gas_usage,
        // 中文注释: 参见下一行代码含义
        ) = if let Some(parent_block) = extending_blocks.last() {
            // 中文注释: 参见下一行代码含义
            let parent_gas_usage = parent_block
                // 中文注释: 参见下一行代码含义
                .as_ref()
                // 中文注释: 参见下一行代码含义
                .validated_txns
                // 中文注释: 参见下一行代码含义
                .iter()
                // 中文注释: 参见下一行代码含义
                .map(|txn| txn.gas_limit())
                // 中文注释: 参见下一行代码含义
                .sum::<u64>();
            // 中文注释: 参见下一行代码含义
            (
                // 中文注释: 参见下一行代码含义
                parent_block.as_ref().header().block_round,
                // 中文注释: 参见下一行代码含义
                parent_block.as_ref().header().base_fee,
                // 中文注释: 参见下一行代码含义
                parent_block.as_ref().header().base_fee_trend,
                // 中文注释: 参见下一行代码含义
                parent_block.as_ref().header().base_fee_moment,
                // 中文注释: 参见下一行代码含义
                parent_gas_usage,
            // 中文注释: 参见下一行代码含义
            )
        // 中文注释: 参见下一行代码含义
        } else {
            // genesis block doesn't exist in committed_cache
            // 中文注释: 参见下一行代码含义
            if self.last_commit == GENESIS_SEQ_NUM {
                // genesis block
                // 中文注释: 参见下一行代码含义
                (
                    // 中文注释: 参见下一行代码含义
                    GENESIS_ROUND,
                    // 中文注释: 参见下一行代码含义
                    Some(monad_tfm::base_fee::GENESIS_BASE_FEE),
                    // 中文注释: 参见下一行代码含义
                    Some(monad_tfm::base_fee::GENESIS_BASE_FEE_TREND),
                    // 中文注释: 参见下一行代码含义
                    Some(monad_tfm::base_fee::GENESIS_BASE_FEE_MOMENT),
                    // 中文注释: 参见下一行代码含义
                    0,
                // 中文注释: 参见下一行代码含义
                )
            // 中文注释: 参见下一行代码含义
            } else {
                // 中文注释: 参见下一行代码含义
                let parent_block = self
                    // 中文注释: 参见下一行代码含义
                    .committed_cache
                    // 中文注释: 参见下一行代码含义
                    .blocks
                    // 中文注释: 参见下一行代码含义
                    .get(&self.last_commit)
                    // 中文注释: 参见下一行代码含义
                    .expect("last committed block must exist");
                // 中文注释: 参见下一行代码含义
                (
                    // 中文注释: 参见下一行代码含义
                    parent_block.round,
                    // 中文注释: 参见下一行代码含义
                    parent_block.base_fee,
                    // 中文注释: 参见下一行代码含义
                    parent_block.base_fee_trend,
                    // 中文注释: 参见下一行代码含义
                    parent_block.base_fee_moment,
                    // 中文注释: 参见下一行代码含义
                    parent_block.block_gas_usage,
                // 中文注释: 参见下一行代码含义
                )
            // 中文注释: 参见下一行代码含义
            }
        // 中文注释: 参见下一行代码含义
        };

        // if parent block doesn't have base_fee fields, it must be pre-tfm
        // block and we return genesis values
        // 中文注释: 参见下一行代码含义
        let (parent_base_fee, parent_trend, parent_moment) = match (
            // 中文注释: 参见下一行代码含义
            maybe_parent_base_fee,
            // 中文注释: 参见下一行代码含义
            maybe_parent_trend,
            // 中文注释: 参见下一行代码含义
            maybe_parent_moment,
        // 中文注释: 参见下一行代码含义
        ) {
            // 中文注释: 参见下一行代码含义
            (Some(parent_base_fee), Some(parent_trend), Some(parent_moment)) => {
                // 中文注释: 参见下一行代码含义
                (parent_base_fee, parent_trend, parent_moment)
            // 中文注释: 参见下一行代码含义
            }
            // 中文注释: 参见下一行代码含义
            _ => (
                // 中文注释: 参见下一行代码含义
                monad_tfm::base_fee::GENESIS_BASE_FEE,
                // 中文注释: 参见下一行代码含义
                monad_tfm::base_fee::GENESIS_BASE_FEE_TREND,
                // 中文注释: 参见下一行代码含义
                monad_tfm::base_fee::GENESIS_BASE_FEE_MOMENT,
            // 中文注释: 参见下一行代码含义
            ),
        // 中文注释: 参见下一行代码含义
        };

        // 中文注释: 参见下一行代码含义
        (
            // 中文注释: 参见下一行代码含义
            parent_block_round,
            // 中文注释: 参见下一行代码含义
            parent_base_fee,
            // 中文注释: 参见下一行代码含义
            parent_trend,
            // 中文注释: 参见下一行代码含义
            parent_moment,
            // 中文注释: 参见下一行代码含义
            parent_gas_usage,
        // 中文注释: 参见下一行代码含义
        )
    // 中文注释: 参见下一行代码含义
    }

    /// compute the base fee according to tfm rules
    ///
    /// return value: (base_fee, base_fee_trend, base_fee_moment)
    ///
    /// base_fee unit: MON-wei
    // 中文注释: 参见下一行代码含义
    pub fn compute_base_fee<B>(
        // 中文注释: 参见下一行代码含义
        &self,
        // 中文注释: 参见下一行代码含义
        extending_blocks: &[B],
        // 中文注释: 参见下一行代码含义
        chain_config: &CCT,
        // 中文注释: 参见下一行代码含义
        timestamp_ns: u128,
    // 中文注释: 参见下一行代码含义
    ) -> Option<(u64, u64, u64)>
    // 中文注释: 参见下一行代码含义
    where
        // 中文注释: 参见下一行代码含义
        B: AsRef<EthValidatedBlock<ST, SCT>>,
    // 中文注释: 参见下一行代码含义
    {
        // 中文注释: 参见下一行代码含义
        let tfm_enabled = chain_config
            // 中文注释: 参见下一行代码含义
            .get_execution_chain_revision(timestamp_ns_to_secs(timestamp_ns))
            // 中文注释: 参见下一行代码含义
            .execution_chain_params()
            // 中文注释: 参见下一行代码含义
            .tfm_enabled;
        // 中文注释: 参见下一行代码含义
        if tfm_enabled {
            // 中文注释: 参见下一行代码含义
            let (
                // 中文注释: 参见下一行代码含义
                parent_block_round,
                // 中文注释: 参见下一行代码含义
                parent_base_fee,
                // 中文注释: 参见下一行代码含义
                parent_trend,
                // 中文注释: 参见下一行代码含义
                parent_moment,
                // 中文注释: 参见下一行代码含义
                parent_gas_usage,
            // 中文注释: 参见下一行代码含义
            ) = self.get_parent_base_fee_fields(extending_blocks);
            // 中文注释: 参见下一行代码含义
            let parent_block_gas_limit = chain_config
                // 中文注释: 参见下一行代码含义
                .get_chain_revision(parent_block_round)
                // 中文注释: 参见下一行代码含义
                .chain_params()
                // 中文注释: 参见下一行代码含义
                .proposal_gas_limit;

            // 中文注释: 参见下一行代码含义
            Some(monad_tfm::base_fee::compute_base_fee(
                // 中文注释: 参见下一行代码含义
                parent_block_gas_limit,
                // 中文注释: 参见下一行代码含义
                parent_gas_usage,
                // 中文注释: 参见下一行代码含义
                parent_base_fee,
                // 中文注释: 参见下一行代码含义
                parent_trend,
                // 中文注释: 参见下一行代码含义
                parent_moment,
            // 中文注释: 参见下一行代码含义
            ))
        // 中文注释: 参见下一行代码含义
        } else {
            // 中文注释: 参见下一行代码含义
            None
        // 中文注释: 参见下一行代码含义
        }
    // 中文注释: 参见下一行代码含义
    }

    // 中文注释: 参见下一行代码含义
    pub fn get_execution_delay(&self) -> SeqNum {
        // 中文注释: 参见下一行代码含义
        self.execution_delay
    // 中文注释: 参见下一行代码含义
    }

    // 中文注释: 参见下一行代码含义
    fn system_transaction_nonce_check(
        // 中文注释: 参见下一行代码含义
        &self,
        // 中文注释: 参见下一行代码含义
        system_txns: &[SystemTransaction],
        // 中文注释: 参见下一行代码含义
        account_nonces: &mut BTreeMap<&Address, u64>,
    // 中文注释: 参见下一行代码含义
    ) -> Result<(), BlockPolicyError> {
        // 中文注释: 参见下一行代码含义
        for sys_txn in system_txns.iter() {
            // 中文注释: 参见下一行代码含义
            let sys_txn_signer = sys_txn.signer();
            // 中文注释: 参见下一行代码含义
            let sys_txn_nonce = sys_txn.nonce();

            // 中文注释: 参见下一行代码含义
            let expected_nonce = account_nonces
                // 中文注释: 参见下一行代码含义
                .get_mut(&sys_txn_signer)
                // 中文注释: 参见下一行代码含义
                .expect("account_nonces should have been populated");

            // 中文注释: 参见下一行代码含义
            if &sys_txn_nonce != expected_nonce {
                // 中文注释: 参见下一行代码含义
                warn!(
                    // 中文注释: 参见下一行代码含义
                    ?sys_txn_nonce,
                    // 中文注释: 参见下一行代码含义
                    ?expected_nonce,
                    // 中文注释: 参见下一行代码含义
                    "block not coherent, invalid nonce for system transaction"
                // 中文注释: 参见下一行代码含义
                );
                // 中文注释: 参见下一行代码含义
                return Err(BlockPolicyError::BlockNotCoherent);
            // 中文注释: 参见下一行代码含义
            }
            // 中文注释: 参见下一行代码含义
            *expected_nonce += 1;
        // 中文注释: 参见下一行代码含义
        }

        // 中文注释: 参见下一行代码含义
        Ok(())
    // 中文注释: 参见下一行代码含义
    }

    // this function checks the validity of nonces for a regular transaction
    // 中文注释: 参见下一行代码含义
    fn nonce_check_and_update(
        // 中文注释: 参见下一行代码含义
        &self,
        // 中文注释: 参见下一行代码含义
        txn: &Recovered<TxEnvelope>,
        // 中文注释: 参见下一行代码含义
        account_nonces: &mut BTreeMap<&Address, u64>,
    // 中文注释: 参见下一行代码含义
    ) -> Result<(), BlockPolicyError> {
        // 中文注释: 参见下一行代码含义
        let eth_address = txn.signer();
        // 中文注释: 参见下一行代码含义
        let txn_nonce = txn.nonce();

        // 中文注释: 参见下一行代码含义
        let expected_nonce = account_nonces
            // 中文注释: 参见下一行代码含义
            .get_mut(&eth_address)
            // 中文注释: 参见下一行代码含义
            .expect("account_nonces should have been populated");

        // 中文注释: 参见下一行代码含义
        if &txn_nonce != expected_nonce {
            // 中文注释: 参见下一行代码含义
            warn!(
                // 中文注释: 参见下一行代码含义
                txn_nonce = ?txn_nonce,
                // 中文注释: 参见下一行代码含义
                expected_nonce = ?expected_nonce,
                // 中文注释: 参见下一行代码含义
                "block not coherent, invalid nonce"
            // 中文注释: 参见下一行代码含义
            );
            // 中文注释: 参见下一行代码含义
            return Err(BlockPolicyError::BlockNotCoherent);
        // 中文注释: 参见下一行代码含义
        }
        // 中文注释: 参见下一行代码含义
        *expected_nonce += 1;

        // 中文注释: 参见下一行代码含义
        Ok(())
    // 中文注释: 参见下一行代码含义
    }

    // https://eips.ethereum.org/EIPS/eip-7702#behavior
    // the nonce of authority is only incremented if the behavior checks
    // for the tuple pass
    // this function performs those checks
    // 中文注释: 参见下一行代码含义
    fn eip_7702_valid_nonce_update(
        // 中文注释: 参见下一行代码含义
        &self,
        // 中文注释: 参见下一行代码含义
        auth_list: &[RecoveredAuthorization],
        // 中文注释: 参见下一行代码含义
        account_nonces: &mut BTreeMap<&Address, u64>,
        // 中文注释: 参见下一行代码含义
        chain_id: u64,
    // 中文注释: 参见下一行代码含义
    ) {
        // 中文注释: 参见下一行代码含义
        for (result, nonce, code_address, auth_chain_id) in auth_list
            // 中文注释: 参见下一行代码含义
            .iter()
            // 中文注释: 参见下一行代码含义
            .map(|a| (a.authority(), a.nonce(), a.address(), a.chain_id()))
        // 中文注释: 参见下一行代码含义
        {
            // 中文注释: 参见下一行代码含义
            match result {
                // 中文注释: 参见下一行代码含义
                Some(authority) => {
                    // 中文注释: 参见下一行代码含义
                    trace!(?code_address, ?nonce, ?authority, "Authority");
                    // 中文注释: 参见下一行代码含义
                    if auth_chain_id != 0_u64 && auth_chain_id != chain_id {
                        // 中文注释: 参见下一行代码含义
                        continue;
                    // 中文注释: 参见下一行代码含义
                    }

                    // 中文注释: 参见下一行代码含义
                    let expected_nonce = account_nonces
                        // 中文注释: 参见下一行代码含义
                        .get_mut(&authority)
                        // 中文注释: 参见下一行代码含义
                        .expect("account_nonces should have been populated");

                    // 中文注释: 参见下一行代码含义
                    if *expected_nonce != nonce {
                        // 中文注释: 参见下一行代码含义
                        trace!(
                            // 中文注释: 参见下一行代码含义
                            ?expected_nonce,
                            // 中文注释: 参见下一行代码含义
                            auth_tuple_nonce = nonce,
                            // 中文注释: 参见下一行代码含义
                            ?authority,
                            // 中文注释: 参见下一行代码含义
                            "authority nonce error"
                        // 中文注释: 参见下一行代码含义
                        );
                        // 中文注释: 参见下一行代码含义
                        continue;
                    // 中文注释: 参见下一行代码含义
                    }
                    // 中文注释: 参见下一行代码含义
                    *expected_nonce += 1;
                // 中文注释: 参见下一行代码含义
                }
                // 中文注释: 参见下一行代码含义
                None => {
                    // skip authorization if there is error recovering signer
                    // 中文注释: 参见下一行代码含义
                    continue;
                // 中文注释: 参见下一行代码含义
                }
            // 中文注释: 参见下一行代码含义
            }
        // 中文注释: 参见下一行代码含义
        }
    // 中文注释: 参见下一行代码含义
    }

    // 中文注释: 参见下一行代码含义
    fn extract_signers(
        // 中文注释: 参见下一行代码含义
        &self,
        // 中文注释: 参见下一行代码含义
        validated_txns: &[ValidatedTx],
        // 中文注释: 参见下一行代码含义
        system_txns: &[SystemTransaction],
    // 中文注释: 参见下一行代码含义
    ) -> Result<(HashSet<Address>, HashSet<Address>), BlockPolicyError> {
        // TODO fix this unnecessary copy into a new vec to generate an owned Address
        // 中文注释: 参见下一行代码含义
        let mut tx_signers: HashSet<Address> =
            // 中文注释: 参见下一行代码含义
            validated_txns.iter().map(|txn| txn.signer()).collect();

        // 中文注释: 参见下一行代码含义
        let authority_addresses: HashSet<Address> = validated_txns
            // 中文注释: 参见下一行代码含义
            .iter()
            // 中文注释: 参见下一行代码含义
            .flat_map(|txn| {
                // 中文注释: 参见下一行代码含义
                txn.authorizations_7702
                    // 中文注释: 参见下一行代码含义
                    .iter()
                    // 中文注释: 参见下一行代码含义
                    .filter_map(|recovered_auth| recovered_auth.authority())
            // 中文注释: 参见下一行代码含义
            })
            // 中文注释: 参见下一行代码含义
            .collect();

        // 中文注释: 参见下一行代码含义
        tx_signers.extend(authority_addresses.iter().cloned());

        // 中文注释: 参见下一行代码含义
        let mut system_tx_signers = system_txns.iter().map(|txn| txn.signer());
        // 中文注释: 参见下一行代码含义
        tx_signers.extend(&mut system_tx_signers);

        // 中文注释: 参见下一行代码含义
        Ok((tx_signers, authority_addresses))
    // 中文注释: 参见下一行代码含义
    }
// 中文注释: 参见下一行代码含义
}

// 中文注释: 参见下一行代码含义
impl<ST, SCT, SBT, CCT, CRT> BlockPolicy<ST, SCT, EthExecutionProtocol, SBT, CCT, CRT>
    // 中文注释: 参见下一行代码含义
    for EthBlockPolicy<ST, SCT, CCT, CRT>
// 中文注释: 参见下一行代码含义
where
    // 中文注释: 参见下一行代码含义
    ST: CertificateSignatureRecoverable,
    // 中文注释: 参见下一行代码含义
    SCT: SignatureCollection<NodeIdPubKey = CertificateSignaturePubKey<ST>>,
    // 中文注释: 参见下一行代码含义
    SBT: StateBackend<ST, SCT>,
    // 中文注释: 参见下一行代码含义
    CCT: ChainConfig<CRT>,
    // 中文注释: 参见下一行代码含义
    CRT: ChainRevision,
// 中文注释: 参见下一行代码含义
{
    // 中文注释: 参见下一行代码含义
    type ValidatedBlock = EthValidatedBlock<ST, SCT>;

    // 中文注释: 参见下一行代码含义
    fn check_coherency(
        // 中文注释: 参见下一行代码含义
        &self,
        // 中文注释: 参见下一行代码含义
        block: &Self::ValidatedBlock,
        // 中文注释: 参见下一行代码含义
        extending_blocks: Vec<&Self::ValidatedBlock>,
        // 中文注释: 参见下一行代码含义
        blocktree_root: RootInfo,
        // 中文注释: 参见下一行代码含义
        state_backend: &SBT,
        // 中文注释: 参见下一行代码含义
        chain_config: &CCT,
    // 中文注释: 参见下一行代码含义
    ) -> Result<(), BlockPolicyError> {
        // 中文注释: 参见下一行代码含义
        let chain_id = chain_config.chain_id();

        // 中文注释: 参见下一行代码含义
        let first_block = extending_blocks
            // 中文注释: 参见下一行代码含义
            .iter()
            // 中文注释: 参见下一行代码含义
            .chain(std::iter::once(&block))
            // 中文注释: 参见下一行代码含义
            .next()
            // 中文注释: 参见下一行代码含义
            .unwrap();
        // 中文注释: 参见下一行代码含义
        assert_eq!(first_block.get_seq_num(), self.last_commit + SeqNum(1));

        // check coherency against the block being extended or against the root of the blocktree if
        // there is no extending branch
        // 中文注释: 参见下一行代码含义
        let (extending_seq_num, extending_timestamp) =
            // 中文注释: 参见下一行代码含义
            if let Some(extended_block) = extending_blocks.last() {
                // 中文注释: 参见下一行代码含义
                (extended_block.get_seq_num(), extended_block.get_timestamp())
            // 中文注释: 参见下一行代码含义
            } else {
                // 中文注释: 参见下一行代码含义
                (blocktree_root.seq_num, 0) //TODO: add timestamp to RootInfo
            // 中文注释: 参见下一行代码含义
            };

        // 中文注释: 参见下一行代码含义
        if block.get_seq_num() != extending_seq_num + SeqNum(1) {
            // 中文注释: 参见下一行代码含义
            warn!(
                // 中文注释: 参见下一行代码含义
                seq_num =? block.header().seq_num,
                // 中文注释: 参见下一行代码含义
                round =? block.header().block_round,
                // 中文注释: 参见下一行代码含义
                "block not coherent, doesn't equal parent_seq_num + 1"
            // 中文注释: 参见下一行代码含义
            );
            // 中文注释: 参见下一行代码含义
            return Err(BlockPolicyError::BlockNotCoherent);
        // 中文注释: 参见下一行代码含义
        }

        // 中文注释: 参见下一行代码含义
        if block.get_timestamp() <= extending_timestamp {
            // 中文注释: 参见下一行代码含义
            warn!(
                // 中文注释: 参见下一行代码含义
                seq_num =? block.header().seq_num,
                // 中文注释: 参见下一行代码含义
                round =? block.header().block_round,
                // 中文注释: 参见下一行代码含义
                ?extending_timestamp,
                // 中文注释: 参见下一行代码含义
                block_timestamp =? block.get_timestamp(),
                // 中文注释: 参见下一行代码含义
                "block not coherent, timestamp not monotonically increasing"
            // 中文注释: 参见下一行代码含义
            );
            // 中文注释: 参见下一行代码含义
            return Err(BlockPolicyError::TimestampError);
        // 中文注释: 参见下一行代码含义
        }

        // 中文注释: 参见下一行代码含义
        let expected_execution_results = self.get_expected_execution_results(
            // 中文注释: 参见下一行代码含义
            block.get_seq_num(),
            // 中文注释: 参见下一行代码含义
            extending_blocks.clone(),
            // 中文注释: 参见下一行代码含义
            state_backend,
        // 中文注释: 参见下一行代码含义
        )?;
        // 中文注释: 参见下一行代码含义
        if block.get_execution_results() != &expected_execution_results {
            // 中文注释: 参见下一行代码含义
            warn!(
                // 中文注释: 参见下一行代码含义
                seq_num =? block.header().seq_num,
                // 中文注释: 参见下一行代码含义
                round =? block.header().block_round,
                // 中文注释: 参见下一行代码含义
                ?expected_execution_results,
                // 中文注释: 参见下一行代码含义
                block_execution_results =? block.get_execution_results(),
                // 中文注释: 参见下一行代码含义
                "block not coherent, execution result mismatch"
            // 中文注释: 参见下一行代码含义
            );
            // 中文注释: 参见下一行代码含义
            return Err(BlockPolicyError::ExecutionResultMismatch);
        // 中文注释: 参见下一行代码含义
        }

        // verify base_fee fields
        // 中文注释: 参见下一行代码含义
        let maybe_tfm_base_fees =
            // 中文注释: 参见下一行代码含义
            self.compute_base_fee(&extending_blocks, chain_config, block.get_timestamp());

        // 中文注释: 参见下一行代码含义
        let (base_fee, base_fee_trend, base_fee_moment) = match maybe_tfm_base_fees {
            // 中文注释: 参见下一行代码含义
            Some((base_fee, base_fee_trend, base_fee_moment)) => {
                // 中文注释: 参见下一行代码含义
                (Some(base_fee), Some(base_fee_trend), Some(base_fee_moment))
            // 中文注释: 参见下一行代码含义
            }
            // 中文注释: 参见下一行代码含义
            None => (None, None, None),
        // 中文注释: 参见下一行代码含义
        };

        // 中文注释: 参见下一行代码含义
        if base_fee != block.header().base_fee
            // 中文注释: 参见下一行代码含义
            || base_fee_trend != block.header().base_fee_trend
            // 中文注释: 参见下一行代码含义
            || base_fee_moment != block.header().base_fee_moment
        // 中文注释: 参见下一行代码含义
        {
            // 中文注释: 参见下一行代码含义
            warn!(
                // 中文注释: 参见下一行代码含义
                seq_num =? block.header().seq_num,
                // 中文注释: 参见下一行代码含义
                round =? block.header().block_round,
                // 中文注释: 参见下一行代码含义
                expected_base_fees = ?(base_fee, base_fee_trend, base_fee_moment),
                // 中文注释: 参见下一行代码含义
                block_base_fees = ?(block.header().base_fee, block.header().base_fee_trend, block.header().base_fee_moment),
                // 中文注释: 参见下一行代码含义
                "block not coherent, base_fee mismatch"
            // 中文注释: 参见下一行代码含义
            );
            // 中文注释: 参见下一行代码含义
            return Err(BlockPolicyError::BaseFeeError);
        // 中文注释: 参见下一行代码含义
        }

        // 中文注释: 参见下一行代码含义
        let (tx_signers, authority_addresses) =
            // 中文注释: 参见下一行代码含义
            self.extract_signers(&block.validated_txns, &block.system_txns)?;

        // these must be updated as we go through txs in the block
        // 中文注释: 参见下一行代码含义
        let mut account_nonces = self.get_account_base_nonces(
            // 中文注释: 参见下一行代码含义
            block.get_seq_num(),
            // 中文注释: 参见下一行代码含义
            state_backend,
            // 中文注释: 参见下一行代码含义
            &extending_blocks,
            // 中文注释: 参见下一行代码含义
            tx_signers.iter(),
        // 中文注释: 参见下一行代码含义
        )?;
        // these must be updated as we go through txs in the block
        // 中文注释: 参见下一行代码含义
        let mut account_balances = self.compute_account_base_balances(
            // 中文注释: 参见下一行代码含义
            block.get_seq_num(),
            // 中文注释: 参见下一行代码含义
            state_backend,
            // 中文注释: 参见下一行代码含义
            chain_config,
            // 中文注释: 参见下一行代码含义
            Some(&extending_blocks),
            // 中文注释: 参见下一行代码含义
            tx_signers.iter(),
        // 中文注释: 参见下一行代码含义
        )?;

        // 中文注释: 参见下一行代码含义
        for authority in &authority_addresses {
            // 中文注释: 参见下一行代码含义
            let account_balance = account_balances
                // 中文注释: 参见下一行代码含义
                .get_mut(authority)
                // 中文注释: 参见下一行代码含义
                .expect("account_balances should have been populated for delegated accounts");

            // 中文注释: 参见下一行代码含义
            trace!(?authority, "Setting account to is_delegated: true");
            // 中文注释: 参见下一行代码含义
            account_balance.is_delegated = true;
        // 中文注释: 参见下一行代码含义
        }

        // 中文注释: 参见下一行代码含义
        let validator = EthBlockPolicyBlockValidator::new(
            // 中文注释: 参见下一行代码含义
            block.get_seq_num(),
            // 中文注释: 参见下一行代码含义
            self.execution_delay,
            // 中文注释: 参见下一行代码含义
            block
                // 中文注释: 参见下一行代码含义
                .get_base_fee()
                // 中文注释: 参见下一行代码含义
                .unwrap_or(monad_tfm::base_fee::PRE_TFM_BASE_FEE),
            // 中文注释: 参见下一行代码含义
            &chain_config.get_chain_revision(block.get_block_round()),
            // 中文注释: 参见下一行代码含义
            &chain_config.get_execution_chain_revision(timestamp_ns_to_secs(block.get_timestamp())),
        // 中文注释: 参见下一行代码含义
        )?;

        // 中文注释: 参见下一行代码含义
        self.system_transaction_nonce_check(&block.system_txns, &mut account_nonces)?;

        // 中文注释: 参见下一行代码含义
        for txn in block.validated_txns.iter() {
            // 中文注释: 参见下一行代码含义
            self.nonce_check_and_update(txn, &mut account_nonces)?;
            // 中文注释: 参见下一行代码含义
            validator.try_add_transaction(&mut account_balances, txn)?;

            // https://eips.ethereum.org/EIPS/eip-7702#behavior
            // "The authorization list is processed before the execution portion
            // of the transaction begins, but after the sender’s nonce is incremented."
            // 中文注释: 参见下一行代码含义
            if txn.is_eip7702() {
                // 中文注释: 参见下一行代码含义
                self.eip_7702_valid_nonce_update(
                    // 中文注释: 参见下一行代码含义
                    &txn.authorizations_7702,
                    // 中文注释: 参见下一行代码含义
                    &mut account_nonces,
                    // 中文注释: 参见下一行代码含义
                    chain_id,
                // 中文注释: 参见下一行代码含义
                );
            // 中文注释: 参见下一行代码含义
            }
        // 中文注释: 参见下一行代码含义
        }

        // 中文注释: 参见下一行代码含义
        Ok(())
    // 中文注释: 参见下一行代码含义
    }

    // 中文注释: 参见下一行代码含义
    fn get_expected_execution_results(
        // 中文注释: 参见下一行代码含义
        &self,
        // 中文注释: 参见下一行代码含义
        block_seq_num: SeqNum,
        // 中文注释: 参见下一行代码含义
        extending_blocks: Vec<&Self::ValidatedBlock>,
        // 中文注释: 参见下一行代码含义
        state_backend: &SBT,
    // 中文注释: 参见下一行代码含义
    ) -> Result<Vec<EthHeader>, StateBackendError> {
        // 中文注释: 参见下一行代码含义
        if block_seq_num < self.execution_delay {
            // 中文注释: 参见下一行代码含义
            return Ok(Vec::new());
        // 中文注释: 参见下一行代码含义
        }
        // 中文注释: 参见下一行代码含义
        let base_seq_num = block_seq_num - self.execution_delay;
        // 中文注释: 参见下一行代码含义
        let block_index = self.get_block_index(&Some(&extending_blocks), &base_seq_num)?;

        // 中文注释: 参见下一行代码含义
        let expected_execution_result = state_backend.get_execution_result(
            // 中文注释: 参见下一行代码含义
            &block_index.block_id,
            // 中文注释: 参见下一行代码含义
            &block_index.seq_num,
            // 中文注释: 参见下一行代码含义
            block_index.is_finalized,
        // 中文注释: 参见下一行代码含义
        )?;

        // 中文注释: 参见下一行代码含义
        Ok(vec![expected_execution_result])
    // 中文注释: 参见下一行代码含义
    }

    // 中文注释: 参见下一行代码含义
    fn update_committed_block(&mut self, block: &Self::ValidatedBlock, chain_config: &CCT) {
        // 中文注释: 参见下一行代码含义
        assert_eq!(block.get_seq_num(), self.last_commit + SeqNum(1));
        // 中文注释: 参见下一行代码含义
        self.last_commit = block.get_seq_num();
        // 中文注释: 参见下一行代码含义
        self.committed_cache.update_committed_block(block);
    // 中文注释: 参见下一行代码含义
    }

    // 中文注释: 参见下一行代码含义
    fn reset(
        // 中文注释: 参见下一行代码含义
        &mut self,
        // 中文注释: 参见下一行代码含义
        last_delay_committed_blocks: Vec<&Self::ValidatedBlock>,
        // 中文注释: 参见下一行代码含义
        chain_config: &CCT,
    // 中文注释: 参见下一行代码含义
    ) {
        // 中文注释: 参见下一行代码含义
        self.committed_cache = CommittedBlkBuffer::new(self.committed_cache.min_buffer_size);
        // 中文注释: 参见下一行代码含义
        for block in last_delay_committed_blocks {
            // 中文注释: 参见下一行代码含义
            self.last_commit = block.get_seq_num();
            // 中文注释: 参见下一行代码含义
            self.committed_cache.update_committed_block(block);
        // 中文注释: 参见下一行代码含义
        }
    // 中文注释: 参见下一行代码含义
    }
// 中文注释: 参见下一行代码含义
}

#[cfg(test)]
mod test {
    use std::collections::HashMap;

    use alloy_consensus::{SignableTransaction, TxEip1559};
    use alloy_eips::eip7702::Authorization;
    use alloy_primitives::{hex, Address, FixedBytes, PrimitiveSignature, TxKind, B256};
    use alloy_signer::SignerSync;
    use alloy_signer_local::PrivateKeySigner;
    use monad_chain_config::{revision::MockChainRevision, MockChainConfig};
    use monad_crypto::NopSignature;
    use monad_eth_testutil::{
        generate_consensus_test_block, make_eip1559_tx_with_value, make_eip7702_tx,
        make_eip7702_tx_with_value, make_signed_authorization, recover_tx, secret_to_eth_address,
        sign_authorization,
    };
    use monad_state_backend::NopStateBackend;
    use monad_testutil::signing::MockSignatures;
    use monad_types::{Balance, Hash, SeqNum};
    use proptest::{prelude::*, strategy::Just};
    use rstest::*;
    use test_case::test_case;

    use super::*;

    const BASE_FEE: u64 = 100_000_000_000;
    const BASE_FEE_TREND: u64 = 0;
    const BASE_FEE_MOMENT: u64 = 0;

    type SignatureType = NopSignature;
    type SignatureCollectionType = MockSignatures<SignatureType>;
    type StateBackendType = NopStateBackend;
    type ChainConfigType = MockChainConfig;
    type ChainRevisionType = MockChainRevision;

    const RESERVE_BALANCE: u128 = 1_000_000_000_000_000_000;
    const EXEC_DELAY: SeqNum = SeqNum(3);

    // pubkey starts with AAA
    const S1: B256 = B256::new(hex!(
        "0ed2e19e3aca1a321349f295837988e9c6f95d4a6fc54cfab6befd5ee82662ad"
    ));
    // pubkey starts with BBB
    const S2: B256 = B256::new(hex!(
        "009ac901cf45a2e92e7e7bdf167dc52e3a6232be3c56cc3b05622b247c2c716a"
    ));

    const ONE_ETHER: u128 = 1_000_000_000_000_000_000;
    const HALF_ETHER: u128 = 500_000_000_000_000_000;
    const CHAIN_ID: u64 = 1337;

    enum CoherencyCheckMode {
        ReserveBalanceCoherency,
        NonceCoherency,
    }

    fn sign_tx(signature_hash: &FixedBytes<32>) -> PrimitiveSignature {
        let secret_key = B256::repeat_byte(0xAu8).to_string();
        let signer = &secret_key.parse::<PrivateKeySigner>().unwrap();
        signer.sign_hash_sync(signature_hash).unwrap()
    }

    fn make_test_tx(
        gas_limit: u64,
        value: u128,
        nonce: u64,
        signer: FixedBytes<32>,
    ) -> Recovered<TxEnvelope> {
        recover_tx(make_eip1559_tx_with_value(
            signer,
            value,
            BASE_FEE as u128,
            0, // priority fee
            gas_limit,
            nonce,
            0, // input length
        ))
    }

    fn make_test_delegation_tx(
        gas_limit: u64,
        value: u128,
        nonce: u64,
        signer: FixedBytes<32>,
        authorizations: HashMap<FixedBytes<32>, Authorization>,
    ) -> Recovered<TxEnvelope> {
        recover_tx(make_eip7702_tx_with_value(
            signer,
            value,
            BASE_FEE as u128,
            0, // priority fee
            gas_limit,
            nonce,
            authorizations
                .into_iter()
                .map(|(authority, authorization)| sign_authorization(authority, authorization))
                .collect(),
            0, // input length
        ))
    }

    pub fn make_test_block(
        round: Round,
        seq_num: SeqNum,
        txs: Vec<Recovered<TxEnvelope>>,
    ) -> EthValidatedBlock<NopSignature, MockSignatures<NopSignature>> {
        let consensus_test_block =
            generate_consensus_test_block(round, seq_num, BASE_FEE, &MockChainConfig::DEFAULT, txs);
        EthValidatedBlock {
            block: consensus_test_block.block,
            system_txns: Vec::new(),
            validated_txns: consensus_test_block.validated_txns,
            nonce_usages: unsafe {
                // Workaround for type resolution failure due to circular dependency
                #[allow(clippy::missing_transmute_annotations)]
                std::mem::transmute(consensus_test_block.nonce_usages)
            },
            txn_fees: consensus_test_block.txn_fees,
        }
    }

    fn reserve_balance_coherency(
        block_policy: EthBlockPolicy<
            SignatureType,
            SignatureCollectionType,
            ChainConfigType,
            ChainRevisionType,
        >,
        incoming_block: EthValidatedBlock<SignatureType, SignatureCollectionType>,
        extending_blocks: Vec<&EthValidatedBlock<SignatureType, SignatureCollectionType>>,
        state_backend: &impl StateBackend<SignatureType, SignatureCollectionType>,
        addresses: Vec<Address>,
    ) -> Result<(), BlockPolicyError> {
        let mut account_balances = block_policy.compute_account_base_balances(
            incoming_block.get_seq_num(),
            state_backend,
            &MockChainConfig::DEFAULT,
            Some(&extending_blocks),
            addresses.iter(),
        )?;

        let validator = EthBlockPolicyBlockValidator::new(
            incoming_block.get_seq_num(),
            block_policy.execution_delay,
            BASE_FEE,
            &MockChainRevision::DEFAULT,
            &MonadExecutionRevision::LATEST,
        )?;

        for txn in incoming_block.validated_txns.iter() {
            validator.try_add_transaction(&mut account_balances, txn)?;
        }

        Ok(())
    }

    fn nonce_coherency(
        block_policy: EthBlockPolicy<
            SignatureType,
            SignatureCollectionType,
            ChainConfigType,
            ChainRevisionType,
        >,
        incoming_block: EthValidatedBlock<SignatureType, SignatureCollectionType>,
        extending_blocks: Vec<&EthValidatedBlock<SignatureType, SignatureCollectionType>>,
        state_backend: &impl StateBackend<SignatureType, SignatureCollectionType>,
        addresses: Vec<Address>,
    ) -> Result<(), BlockPolicyError> {
        let mut account_nonces = block_policy.get_account_base_nonces(
            incoming_block.get_seq_num(),
            state_backend,
            &extending_blocks,
            addresses.iter(),
        )?;

        for txn in incoming_block.validated_txns.iter() {
            block_policy.nonce_check_and_update(txn, &mut account_nonces)?;
            if txn.is_eip7702() {
                block_policy.eip_7702_valid_nonce_update(
                    &txn.authorizations_7702,
                    &mut account_nonces,
                    CHAIN_ID,
                );
            }
        }

        Ok(())
    }

    fn setup_block_policy_with_txs(
        txs: BTreeMap<u64, Vec<Recovered<TxEnvelope>>>,
        signers: Vec<Address>,
        state_backend: &impl StateBackend<SignatureType, SignatureCollectionType>,
        num_committed_blocks: usize,
        coherency_check_mode: CoherencyCheckMode,
    ) -> Result<(), BlockPolicyError> {
        let mut block_policy = EthBlockPolicy::<
            SignatureType,
            SignatureCollectionType,
            ChainConfigType,
            ChainRevisionType,
        >::new(SeqNum(17), EXEC_DELAY.0);

        // Build 5 sequential blocks (n-4 .. n)
        let seq_num = 18;
        let mut blocks = Vec::new();
        for offset in 0..=4 {
            let seq = seq_num + offset;
            let txs = txs.get(&offset).cloned().unwrap_or_default();
            let block = make_test_block(Round(1), SeqNum(seq), txs);
            blocks.push(block);
        }

        // Commit blocks
        for block in &blocks[0..num_committed_blocks] {
            BlockPolicy::<_, _, _, StateBackendType, _, _>::update_committed_block(
                &mut block_policy,
                block,
                &MockChainConfig::DEFAULT,
            );
        }

        // Last block is incoming_block
        // Remaining ones in the middle are extending_block
        let incoming_block = blocks[4].clone();
        let extending_blocks = blocks[num_committed_blocks..4].iter().collect();

        match coherency_check_mode {
            CoherencyCheckMode::ReserveBalanceCoherency => reserve_balance_coherency(
                block_policy,
                incoming_block,
                extending_blocks,
                state_backend,
                signers,
            ),
            CoherencyCheckMode::NonceCoherency => nonce_coherency(
                block_policy,
                incoming_block,
                extending_blocks,
                state_backend,
                signers,
            ),
        }
    }

    #[test_case(3; "three committed blocks, one extending block")]
    #[test_case(0; "no committed blocks, four extending block")]
    fn test_check_reserve_balance_coherency(num_committed_blocks: usize) {
        //////////////////////////////////////////////////////////////////
        // Case1: Single emptying transaction                          ///
        //////////////////////////////////////////////////////////////////

        let tx1 = make_test_tx(50000, HALF_ETHER, 0, S1);
        let signer = tx1.signer();
        let txs = BTreeMap::from([(4, vec![tx1])]); // tx in block n

        // balance of signer at block n-3
        // minimum balance required is gas limit * gas bid
        let gas_cost = 50000 * BASE_FEE as u128;
        let state_backend = NopStateBackend {
            balances: BTreeMap::from([(signer, U256::from(gas_cost))]),
            ..Default::default()
        };

        let result = setup_block_policy_with_txs(
            txs.clone(),
            vec![signer],
            &state_backend,
            num_committed_blocks,
            CoherencyCheckMode::ReserveBalanceCoherency,
        );
        assert!(result.is_ok(), "Block coherency check failed: {:?}", result);

        // should return error if fall below minimum balance
        let state_backend = NopStateBackend {
            balances: BTreeMap::from([(signer, U256::from(gas_cost - 1))]),
            ..Default::default()
        };
        let result = setup_block_policy_with_txs(
            txs,
            vec![signer],
            &state_backend,
            num_committed_blocks,
            CoherencyCheckMode::ReserveBalanceCoherency,
        );
        assert!(
            result.is_err(),
            "Block coherency check should have failed: {:?}",
            result
        );

        ///////////////////////////////////////////////////////////////////////////////////
        // Case2: Emptying transaction + another transaction in same block              ///
        ///////////////////////////////////////////////////////////////////////////////////

        // first tx dips into reserve balance, second tx has gas cost less than remaining reserve balance
        let tx1 = make_test_tx(50000, ONE_ETHER, 0, S1);
        let tx2 = make_test_tx(50000, HALF_ETHER, 1, S1);
        let txs = BTreeMap::from([(4, vec![tx1, tx2])]); // txs in block n

        // balance of signer at block n-3
        let state_backend = NopStateBackend {
            balances: BTreeMap::from([(signer, U256::from(ONE_ETHER + HALF_ETHER))]),
            ..Default::default()
        };

        let result = setup_block_policy_with_txs(
            txs,
            vec![signer],
            &state_backend,
            num_committed_blocks,
            CoherencyCheckMode::ReserveBalanceCoherency,
        );
        assert!(result.is_ok(), "Block coherency check failed: {:?}", result);

        // first tx dips into reserve balance, second tx has gas cost more than remaining reserve balance
        let tx1 = make_test_tx(50000, ONE_ETHER, 0, S1);
        let tx2 = make_test_tx(50000, HALF_ETHER, 1, S1);
        let txs = BTreeMap::from([(4, vec![tx1, tx2])]); // txs in block n

        // balance of signer at block n-3
        let gas_cost = 50000 * BASE_FEE as u128;
        let balance = ONE_ETHER + (2 * gas_cost) - 1;
        let state_backend = NopStateBackend {
            balances: BTreeMap::from([(signer, U256::from(balance))]),
            ..Default::default()
        };

        let result = setup_block_policy_with_txs(
            txs,
            vec![signer],
            &state_backend,
            num_committed_blocks,
            CoherencyCheckMode::ReserveBalanceCoherency,
        );
        assert!(
            result.is_err(),
            "Block coherency check should have failed: {:?}",
            result
        );

        // first tx doesn't dip into reserve balance, second tx has max reserve balance to spend from
        let tx1 = make_test_tx(50000, 0, 0, S1);
        let tx2 = make_test_tx(10_000_000, HALF_ETHER, 1, S1);
        let txs = BTreeMap::from([(4, vec![tx1, tx2.clone()])]); // txs in block n

        // balance of signer at block n-3
        assert_eq!(tx2.gas_limit() as u128 * BASE_FEE as u128, RESERVE_BALANCE);
        let first_tx_gas_cost = 50000 * BASE_FEE as u128;
        let second_tx_gas_cost = RESERVE_BALANCE;
        let balance = first_tx_gas_cost + second_tx_gas_cost;
        let state_backend = NopStateBackend {
            balances: BTreeMap::from([(signer, U256::from(balance))]),
            ..Default::default()
        };

        let result = setup_block_policy_with_txs(
            txs,
            vec![signer],
            &state_backend,
            num_committed_blocks,
            CoherencyCheckMode::ReserveBalanceCoherency,
        );
        assert!(result.is_ok(), "Block coherency check failed: {:?}", result);

        ///////////////////////////////////////////////////////////////////////////////////
        // Case3: Emptying transaction + another transaction in different block         ///
        ///////////////////////////////////////////////////////////////////////////////////

        // first tx dips into reserve balance, second tx has gas cost less than remaining reserve balance
        let tx1 = make_test_tx(50000, ONE_ETHER, 0, S1);
        let tx2 = make_test_tx(50000, HALF_ETHER, 1, S1);
        // first tx in block n-2, second tx in block n
        let txs = BTreeMap::from([(2, vec![tx1]), (4, vec![tx2])]);

        // balance of signer at block n-3
        let state_backend = NopStateBackend {
            balances: BTreeMap::from([(signer, U256::from(ONE_ETHER + HALF_ETHER))]),
            ..Default::default()
        };

        let result = setup_block_policy_with_txs(
            txs,
            vec![signer],
            &state_backend,
            num_committed_blocks,
            CoherencyCheckMode::ReserveBalanceCoherency,
        );
        assert!(result.is_ok(), "Block coherency check failed: {:?}", result);

        // first tx dips into reserve balance, second tx has gas cost more than remaining reserve balance
        let tx1 = make_test_tx(50000, ONE_ETHER, 0, S1);
        let tx2 = make_test_tx(50000, HALF_ETHER, 1, S1);
        // first tx in block n-2, second tx in block n
        let txs = BTreeMap::from([(2, vec![tx1]), (4, vec![tx2])]);

        // balance of signer at block n-3
        let gas_cost = 50000 * BASE_FEE as u128;
        let balance = ONE_ETHER + (2 * gas_cost) - 1;
        let state_backend = NopStateBackend {
            balances: BTreeMap::from([(signer, U256::from(balance))]),
            ..Default::default()
        };

        let result = setup_block_policy_with_txs(
            txs,
            vec![signer],
            &state_backend,
            num_committed_blocks,
            CoherencyCheckMode::ReserveBalanceCoherency,
        );
        assert!(
            result.is_err(),
            "Block coherency check should have failed: {:?}",
            result
        );

        // first tx doesn't dip into reserve balance, second tx has max reserve balance to spend from
        let tx1 = make_test_tx(50000, 0, 0, S1);
        let tx2 = make_test_tx(10_000_000, HALF_ETHER, 1, S1);
        // first tx in block n-2, second tx in block n
        let txs = BTreeMap::from([(2, vec![tx1]), (4, vec![tx2.clone()])]);

        // balance of signer at block n-3
        assert_eq!(tx2.gas_limit() as u128 * BASE_FEE as u128, RESERVE_BALANCE);
        let first_tx_gas_cost = 50000 * BASE_FEE as u128;
        let second_tx_gas_cost = RESERVE_BALANCE;
        let balance = first_tx_gas_cost + second_tx_gas_cost;
        let state_backend = NopStateBackend {
            balances: BTreeMap::from([(signer, U256::from(balance))]),
            ..Default::default()
        };

        let result = setup_block_policy_with_txs(
            txs,
            vec![signer],
            &state_backend,
            num_committed_blocks,
            CoherencyCheckMode::ReserveBalanceCoherency,
        );
        assert!(result.is_ok(), "Block coherency check failed: {:?}", result);

        ///////////////////////////////////////////////////////////////////////////////////
        // Case4: Non-emptying transaction + another transaction in different block     ///
        ///////////////////////////////////////////////////////////////////////////////////

        // only gas cost of transactions are taken into account, txn value is not included when calculating reserve balance
        let tx1 = make_test_tx(50000, ONE_ETHER, 0, S1);
        let tx2 = make_test_tx(50000, HALF_ETHER, 1, S1);
        let tx3 = make_test_tx(50000, HALF_ETHER, 2, S1);
        // first tx in block n-3, second tx in block n-2, third tx in block n
        let txs = BTreeMap::from([(1, vec![tx1]), (2, vec![tx2]), (4, vec![tx3])]);

        // balance of signer at block n-3
        let gas_cost = 50000 * 2 * BASE_FEE as u128;
        let state_backend = NopStateBackend {
            balances: BTreeMap::from([(signer, U256::from(gas_cost))]),
            ..Default::default()
        };

        let result = setup_block_policy_with_txs(
            txs,
            vec![signer],
            &state_backend,
            num_committed_blocks,
            CoherencyCheckMode::ReserveBalanceCoherency,
        );
        assert!(result.is_ok(), "Block coherency check failed: {:?}", result);

        // transactions exceed reserve balance
        let tx1 = make_test_tx(50000, ONE_ETHER, 0, S1);
        let tx2 = make_test_tx(50000, HALF_ETHER, 1, S1);
        let tx3 = make_test_tx(50001, HALF_ETHER, 2, S1);
        // first tx in block n-3, second tx in block n-2, third tx in block n
        let txs = BTreeMap::from([(1, vec![tx1]), (2, vec![tx2]), (4, vec![tx3])]);

        // balance of signer at block n-3
        let gas_cost = 50000 * 2 * BASE_FEE as u128;
        let state_backend = NopStateBackend {
            balances: BTreeMap::from([(signer, U256::from(gas_cost))]),
            ..Default::default()
        };

        let result = setup_block_policy_with_txs(
            txs,
            vec![signer],
            &state_backend,
            num_committed_blocks,
            CoherencyCheckMode::ReserveBalanceCoherency,
        );
        assert!(
            result.is_err(),
            "Block coherency check should have failed: {:?}",
            result
        );

        //////////////////////////////////////////////////////////////////////////////////////////////////////
        // Case5: Non-emptying transaction (7702 delegations) + another transaction in different block     ///
        //////////////////////////////////////////////////////////////////////////////////////////////////////

        // first tx has an authorization from the signer
        let tx1 = make_test_delegation_tx(
            50000,
            0,
            0,
            S2,
            HashMap::from([(
                S1,
                Authorization {
                    chain_id: CHAIN_ID,
                    nonce: 0,
                    address: Address(FixedBytes([0x11; 20])),
                },
            )]),
        );
        let tx2 = make_test_tx(50000, HALF_ETHER, 1, S1);
        let tx3 = make_test_tx(50000, HALF_ETHER, 2, S1);
        // first tx in block n-3, second tx in block n-2, third tx in block n
        let txs = BTreeMap::from([(1, vec![tx1]), (2, vec![tx2]), (4, vec![tx3])]);

        // balance of signer at block n-3
        let gas_cost = 50000 * 2 * BASE_FEE as u128;
        let state_backend = NopStateBackend {
            balances: BTreeMap::from([(signer, U256::from(gas_cost))]),
            ..Default::default()
        };

        let result = setup_block_policy_with_txs(
            txs,
            vec![signer],
            &state_backend,
            num_committed_blocks,
            CoherencyCheckMode::ReserveBalanceCoherency,
        );
        assert!(result.is_ok(), "Block coherency check failed: {:?}", result);

        //////////////////////////////////////////////////////////////////////////
        // Case6: Emptying transaction (with another 7702 delegations)         ///
        //////////////////////////////////////////////////////////////////////////

        // first tx has an authorization from the signer
        let tx1 = make_test_delegation_tx(
            50000,
            0,
            0,
            S2,
            HashMap::from([(
                S1,
                Authorization {
                    chain_id: CHAIN_ID,
                    nonce: 0,
                    address: Address(FixedBytes([0x11; 20])),
                },
            )]),
        );
        let tx2 = make_test_tx(50000, HALF_ETHER, 1, S1);
        let signer1 = tx1.signer();
        let signer2 = tx2.signer();
        let txs = BTreeMap::from([(4, vec![tx1, tx2])]);

        // balance of signer at block n-3
        let gas_cost = 50000 * BASE_FEE as u128;
        let state_backend = NopStateBackend {
            balances: BTreeMap::from([
                (signer1, U256::from(gas_cost)),
                (signer2, U256::from(gas_cost)),
            ]),
            ..Default::default()
        };

        let result = setup_block_policy_with_txs(
            txs,
            vec![signer1, signer2],
            &state_backend,
            num_committed_blocks,
            CoherencyCheckMode::ReserveBalanceCoherency,
        );
        assert!(result.is_ok(), "Block coherency check failed: {:?}", result);

        /////////////////////////////////////////////////////////////////////////////
        // Case7: Multiple 7702 delegations and attempted emptying transaction    ///
        /////////////////////////////////////////////////////////////////////////////

        // first tx has multiple delegation and undelegation authorization from the signer
        let tx1 = make_test_delegation_tx(
            50000,
            0,
            0,
            S2,
            HashMap::from([
                (
                    S1,
                    Authorization {
                        chain_id: CHAIN_ID,
                        nonce: 0,
                        address: Address(FixedBytes([0x11; 20])),
                    },
                ),
                (
                    S1,
                    Authorization {
                        chain_id: CHAIN_ID,
                        nonce: 1,
                        address: Address::ZERO,
                    },
                ),
            ]),
        );
        let tx2 = make_test_tx(50000, HALF_ETHER, 2, S1);
        let signer1 = tx1.signer();
        let signer2 = tx2.signer();
        let txs = BTreeMap::from([(4, vec![tx1, tx2])]);

        // balance of signer at block n-3
        let gas_cost = 50000 * BASE_FEE as u128;
        let state_backend = NopStateBackend {
            balances: BTreeMap::from([
                (signer1, U256::from(gas_cost)),
                (signer2, U256::from(gas_cost)),
            ]),
            ..Default::default()
        };

        let result = setup_block_policy_with_txs(
            txs,
            vec![signer1, signer2],
            &state_backend,
            num_committed_blocks,
            CoherencyCheckMode::ReserveBalanceCoherency,
        );
        assert!(result.is_ok(), "Block coherency check failed: {:?}", result);
    }

    #[test_case(3; "three committed blocks, one extending block")]
    #[test_case(0; "no committed blocks, four extending block")]
    fn test_check_nonce_coherency(num_committed_blocks: usize) {
        //////////////////////////////////////////////////////////////////
        // Case1: No 7702 txs                                          ///
        //////////////////////////////////////////////////////////////////

        let tx1 = make_test_tx(50000, 0, 0, S1);
        let tx2 = make_test_tx(50000, 0, 1, S1);
        let tx3 = make_test_tx(50000, 0, 2, S1);
        let tx4 = make_test_tx(50000, 0, 3, S1);
        let signer = tx1.signer();
        let txs = BTreeMap::from([(2, vec![tx1]), (3, vec![tx2, tx3]), (4, vec![tx4])]);

        // balance of signer at block n-3
        let state_backend = NopStateBackend {
            balances: BTreeMap::from([(signer, U256::from(ONE_ETHER))]),
            ..Default::default()
        };

        let result = setup_block_policy_with_txs(
            txs,
            vec![signer],
            &state_backend,
            num_committed_blocks,
            CoherencyCheckMode::NonceCoherency,
        );
        assert!(result.is_ok(), "Block coherency check failed: {:?}", result);

        //////////////////////////////////////////////////////////////////
        // Case2: 7702 txs in incoming block                           ///
        //////////////////////////////////////////////////////////////////

        // correct sequencing of nonces
        let tx1 = make_test_tx(50000, 0, 0, S1);
        let tx2 = make_test_delegation_tx(
            50000,
            0,
            0,
            S2,
            HashMap::from([(
                S1,
                Authorization {
                    chain_id: CHAIN_ID,
                    nonce: 1,
                    address: Address(FixedBytes([0x11; 20])),
                },
            )]),
        );
        let tx3 = make_test_tx(50000, 0, 2, S1);
        let signer1 = tx1.signer();
        let signer2 = tx2.signer();
        let txs = BTreeMap::from([(2, vec![tx1]), (4, vec![tx2, tx3])]);

        // balance of signer at block n-3
        let state_backend = NopStateBackend {
            balances: BTreeMap::from([(signer1, U256::from(ONE_ETHER))]),
            nonces: BTreeMap::from([(signer1, 0), (signer2, 0)]),
        };

        let result = setup_block_policy_with_txs(
            txs,
            vec![signer1, signer2],
            &state_backend,
            num_committed_blocks,
            CoherencyCheckMode::NonceCoherency,
        );
        assert!(result.is_ok(), "Block coherency check failed: {:?}", result);

        // incorrect sequencing of nonces -- tx3 has incorrect nonce
        let tx1 = make_test_tx(50000, 0, 0, S1);
        let tx2 = make_test_delegation_tx(
            50000,
            0,
            0,
            S2,
            HashMap::from([(
                S1,
                Authorization {
                    chain_id: CHAIN_ID,
                    nonce: 1,
                    address: Address(FixedBytes([0x11; 20])),
                },
            )]),
        );
        let tx3 = make_test_tx(50000, 0, 1, S1);
        let signer1 = tx1.signer();
        let signer2 = tx2.signer();
        let txs = BTreeMap::from([(2, vec![tx1]), (4, vec![tx2, tx3])]);

        // balance of signer at block n-3
        let state_backend = NopStateBackend {
            balances: BTreeMap::from([(signer1, U256::from(ONE_ETHER))]),
            nonces: BTreeMap::from([(signer1, 0), (signer2, 0)]),
        };

        let result = setup_block_policy_with_txs(
            txs,
            vec![signer1, signer2],
            &state_backend,
            num_committed_blocks,
            CoherencyCheckMode::NonceCoherency,
        );
        assert!(
            result.is_err(),
            "Block coherency check should have failed: {:?}",
            result
        );

        // incorrect nonce in authorization -- shouldn't affect coherency
        let tx1 = make_test_tx(50000, 0, 0, S1);
        let tx2 = make_test_delegation_tx(
            50000,
            0,
            0,
            S2,
            HashMap::from([(
                S1,
                Authorization {
                    chain_id: CHAIN_ID,
                    nonce: 2,
                    address: Address(FixedBytes([0x11; 20])),
                },
            )]),
        );
        let tx3 = make_test_tx(50000, 0, 1, S1);
        let signer1 = tx1.signer();
        let signer2 = tx2.signer();
        let txs = BTreeMap::from([(2, vec![tx1]), (4, vec![tx2, tx3])]);

        // balance of signer at block n-3
        let state_backend = NopStateBackend {
            balances: BTreeMap::from([(signer1, U256::from(ONE_ETHER))]),
            nonces: BTreeMap::from([(signer1, 0), (signer2, 0)]),
        };

        let result = setup_block_policy_with_txs(
            txs,
            vec![signer1, signer2],
            &state_backend,
            num_committed_blocks,
            CoherencyCheckMode::NonceCoherency,
        );
        assert!(result.is_ok(), "Block coherency check failed: {:?}", result);

        //////////////////////////////////////////////////////////////////
        // Case3: 7702 txs in committed and extending blocks           ///
        //////////////////////////////////////////////////////////////////

        let tx1 = make_test_tx(50000, 0, 0, S1);
        let tx2 = make_test_delegation_tx(
            50000,
            0,
            0,
            S2,
            HashMap::from([(
                S1,
                Authorization {
                    chain_id: CHAIN_ID,
                    nonce: 1,
                    address: Address(FixedBytes([0x11; 20])),
                },
            )]),
        );
        let signer1 = tx1.signer();
        let signer2 = tx2.signer();

        // coherent incoming block
        let tx3 = make_test_tx(50000, 0, 2, S1);
        let txs = BTreeMap::from([
            (2, vec![tx1.clone()]),
            (3, vec![tx2.clone()]),
            (4, vec![tx3]),
        ]);

        // balance of signer at block n-3
        let state_backend = NopStateBackend {
            balances: BTreeMap::from([(signer1, U256::from(ONE_ETHER))]),
            nonces: BTreeMap::from([(signer1, 0), (signer2, 0)]),
        };

        let result = setup_block_policy_with_txs(
            txs,
            vec![signer1, signer2],
            &state_backend,
            num_committed_blocks,
            CoherencyCheckMode::NonceCoherency,
        );
        assert!(result.is_ok(), "Block coherency check failed: {:?}", result);

        // incoherent incoming block
        let tx3 = make_test_tx(50000, 0, 3, S1);
        let txs = BTreeMap::from([(2, vec![tx1]), (3, vec![tx2]), (4, vec![tx3])]);

        // balance of signer at block n-3
        let state_backend = NopStateBackend {
            balances: BTreeMap::from([(signer1, U256::from(ONE_ETHER))]),
            nonces: BTreeMap::from([(signer1, 0), (signer2, 0)]),
        };

        let result = setup_block_policy_with_txs(
            txs,
            vec![signer1, signer2],
            &state_backend,
            num_committed_blocks,
            CoherencyCheckMode::NonceCoherency,
        );
        assert!(
            result.is_err(),
            "Block coherency check should have failed: {:?}",
            result
        );
    }

    #[test]
    fn test_compute_account_balance_state() {
        // setup test addresses
        let address1 = Address(FixedBytes([0x11; 20]));
        let address2 = Address(FixedBytes([0x22; 20]));
        let address3 = Address(FixedBytes([0x33; 20]));

        let max_reserve_balance = Balance::from(RESERVE_BALANCE);

        // add committed blocks to buffer
        let mut buffer = CommittedBlkBuffer::<
            SignatureType,
            SignatureCollectionType,
            MockChainConfig,
            MockChainRevision,
        >::new(3);
        let block1 = CommittedBlock {
            block_id: BlockId(Hash(Default::default())),
            round: Round(0),
            epoch: Epoch(1),
            seq_num: SeqNum(1),
            timestamp_ns: 1,
            nonce_usages: NonceUsageMap {
                map: BTreeMap::from([
                    (address1, NonceUsage::Known(1)),
                    (address2, NonceUsage::Known(1)),
                ]),
            },
            fees: BlockTxnFeeStates {
                txn_fees: BTreeMap::from([
                    (
                        address1,
                        TxnFee {
                            first_txn_value: Balance::from(100),
                            first_txn_gas: Balance::from(10),
                            max_gas_cost: Balance::from(90),
                            max_txn_cost: Balance::ZERO,
                            is_delegated: false,
                        },
                    ),
                    (
                        address2,
                        TxnFee {
                            first_txn_value: Balance::from(200),
                            first_txn_gas: Balance::from(10),
                            max_gas_cost: Balance::from(190),
                            max_txn_cost: Balance::ZERO,
                            is_delegated: false,
                        },
                    ),
                ]),
            },
            base_fee: Some(BASE_FEE),
            base_fee_trend: Some(BASE_FEE_TREND),
            base_fee_moment: Some(BASE_FEE_MOMENT),
            block_gas_usage: 0, // not used in this test
        };

        let block2 = CommittedBlock {
            block_id: BlockId(Hash(Default::default())),
            round: Round(0),
            epoch: Epoch(1),
            timestamp_ns: 1,
            seq_num: SeqNum(2),
            nonce_usages: NonceUsageMap {
                map: BTreeMap::from([
                    (address1, NonceUsage::Known(2)),
                    (address3, NonceUsage::Known(1)),
                ]),
            },
            fees: BlockTxnFeeStates {
                txn_fees: BTreeMap::from([
                    (
                        address1,
                        TxnFee {
                            first_txn_value: Balance::from(150),
                            first_txn_gas: Balance::from(10),
                            max_gas_cost: Balance::from(140),
                            max_txn_cost: Balance::ZERO,
                            is_delegated: false,
                        },
                    ),
                    (
                        address3,
                        TxnFee {
                            first_txn_value: Balance::from(300),
                            first_txn_gas: Balance::from(10),
                            max_gas_cost: Balance::from(290),
                            max_txn_cost: Balance::ZERO,
                            is_delegated: false,
                        },
                    ),
                ]),
            },
            base_fee: Some(BASE_FEE),
            base_fee_trend: Some(BASE_FEE_TREND),
            base_fee_moment: Some(BASE_FEE_MOMENT),
            block_gas_usage: 0, // not used in this test
        };

        let block3 = CommittedBlock {
            block_id: BlockId(Hash(Default::default())),
            round: Round(0),
            epoch: Epoch(1),
            seq_num: SeqNum(3),
            timestamp_ns: 1,
            nonce_usages: NonceUsageMap {
                map: BTreeMap::from([
                    (address2, NonceUsage::Known(2)),
                    (address3, NonceUsage::Known(2)),
                ]),
            },
            fees: BlockTxnFeeStates {
                txn_fees: BTreeMap::from([
                    (
                        address2,
                        TxnFee {
                            first_txn_value: Balance::from(250),
                            first_txn_gas: Balance::from(10),
                            max_gas_cost: Balance::from(240),
                            max_txn_cost: Balance::ZERO,
                            is_delegated: false,
                        },
                    ),
                    (
                        address3,
                        TxnFee {
                            first_txn_value: Balance::from(350),
                            first_txn_gas: Balance::from(10),
                            max_gas_cost: Balance::from(0),
                            max_txn_cost: Balance::ZERO,
                            is_delegated: false,
                        },
                    ),
                ]),
            },
            base_fee: Some(BASE_FEE),
            base_fee_trend: Some(BASE_FEE_TREND),
            base_fee_moment: Some(BASE_FEE_MOMENT),
            block_gas_usage: 0, // not used in this test
        };

        buffer.blocks.insert(SeqNum(1), block1);
        buffer.blocks.insert(SeqNum(2), block2);
        buffer.blocks.insert(SeqNum(3), block3);

        // committed blocks are out of range for emptying and reserve balance check
        let mut account_balance_address_1 = AccountBalanceState {
            balance: Balance::from(250),
            block_seqnum_of_latest_txn: GENESIS_SEQ_NUM,
            remaining_reserve_balance: Balance::from(250),
            max_reserve_balance,
            is_delegated: false,
        };
        let res = buffer.update_account_balance(
            &mut account_balance_address_1,
            &address1,
            EXEC_DELAY,
            SeqNum(4)..SeqNum(5),
            SeqNum(5)..,
            &MockChainConfig::DEFAULT,
        );
        assert!(res.is_ok());

        let emptying_txn_check_block_range = SeqNum(2)..SeqNum(3);
        let reserve_balance_check_block_range = SeqNum(3)..;

        let mut account_balance_address_2 = AccountBalanceState {
            balance: Balance::from(250),
            block_seqnum_of_latest_txn: GENESIS_SEQ_NUM,
            remaining_reserve_balance: Balance::from(250),
            max_reserve_balance,
            is_delegated: false,
        };
        let res = buffer.update_account_balance(
            &mut account_balance_address_2,
            &address2,
            EXEC_DELAY,
            emptying_txn_check_block_range.clone(),
            reserve_balance_check_block_range.clone(),
            &MockChainConfig::DEFAULT,
        );
        // no transaction in block2 (emptying transaction check)
        // gas cost + value more than balance in block3 (reserve balance check)
        assert_eq!(
            res,
            Err(BlockPolicyError::BlockPolicyBlockValidatorError(
                BlockPolicyBlockValidatorError::InsufficientReserveBalance
            ))
        );

        let mut account_balance_address_3 = AccountBalanceState {
            balance: Balance::from(250),
            block_seqnum_of_latest_txn: GENESIS_SEQ_NUM,
            remaining_reserve_balance: Balance::from(250),
            max_reserve_balance,
            is_delegated: false,
        };
        let res = buffer.update_account_balance(
            &mut account_balance_address_3,
            &address3,
            EXEC_DELAY,
            emptying_txn_check_block_range,
            reserve_balance_check_block_range,
            &MockChainConfig::DEFAULT,
        );
        // has a transaction in block2 (emptying transaction check)
        // gas cost more than balance in block3 (reserve balance check)
        assert!(res.is_ok());
        assert_eq!(
            account_balance_address_3.remaining_reserve_balance,
            Balance::from(240)
        );
    }

    proptest! {
        #[test]
        fn test_compute_txn_max_value_no_overflow(
            gas_limit in 0u64..=u64::MAX,
            max_fee_per_gas in 0u128..=u128::MAX,
            value in prop_oneof![
                Just(U256::ZERO),
                Just(U256::MAX),
                any::<[u8; 32]>().prop_map(U256::from_be_bytes)
            ]
        ) {
            let tx = TxEip1559 {
                chain_id: 1337,
                nonce: 0,
                to: TxKind::Call(Address(FixedBytes([0x11; 20]))),
                max_fee_per_gas,
                max_priority_fee_per_gas: max_fee_per_gas,
                gas_limit,
                value,
                ..Default::default()
            };
            let signature = sign_tx(&tx.signature_hash());
            let tx_envelope = TxEnvelope::from(tx.into_signed(signature));

            let result = compute_txn_max_value(&tx_envelope, BASE_FEE);

            let gas_cost_u256 = U256::from(gas_limit).checked_mul(U256::from(max_fee_per_gas)).expect("overflow should not occur with U256overflow should not occur with U256");
            let expected_max_value = U256::from(value).saturating_add(gas_cost_u256);
            assert_eq!(result, expected_max_value);
        }
    }

    #[test]
    fn test_validate_emptying_txn() {
        let reserve_balance = Balance::from(RESERVE_BALANCE);
        let latest_seq_num = SeqNum(1000);
        let txn_value = 1000;
        let block_seq_num = latest_seq_num + EXEC_DELAY;

        let tx = make_test_tx(50000, txn_value, 0, S1);
        let txs = vec![tx.clone()];
        let signer = tx.recover_signer().unwrap();
        let min_balance = compute_txn_max_gas_cost(&tx, BASE_FEE);

        let mut account_balances: BTreeMap<&Address, AccountBalanceState> = BTreeMap::new();
        account_balances.insert(
            &signer,
            AccountBalanceState {
                balance: min_balance,
                remaining_reserve_balance: min_balance,
                block_seqnum_of_latest_txn: latest_seq_num,
                max_reserve_balance: reserve_balance,
                is_delegated: false,
            },
        );

        let validator = EthBlockPolicyBlockValidator::new(
            block_seq_num,
            EXEC_DELAY,
            BASE_FEE,
            &MockChainRevision::DEFAULT,
            &MonadExecutionRevision::LATEST,
        )
        .unwrap();

        for txn in txs.iter() {
            assert!(validator
                .try_add_transaction(&mut account_balances, txn)
                .is_ok());
        }

        let mut account_balances: BTreeMap<&Address, AccountBalanceState> = BTreeMap::new();
        account_balances.insert(
            &signer,
            AccountBalanceState {
                balance: min_balance - Balance::from(1),
                remaining_reserve_balance: min_balance,
                block_seqnum_of_latest_txn: latest_seq_num,
                max_reserve_balance: reserve_balance,
                is_delegated: false,
            },
        );

        let validator = EthBlockPolicyBlockValidator::new(
            block_seq_num,
            EXEC_DELAY,
            BASE_FEE,
            &MockChainRevision::DEFAULT,
            &MonadExecutionRevision::LATEST,
        )
        .unwrap();

        for txn in txs.iter() {
            assert!(
                validator.try_add_transaction(&mut account_balances, txn)
                    == Err(BlockPolicyError::BlockPolicyBlockValidatorError(
                        BlockPolicyBlockValidatorError::InsufficientBalance
                    ))
            );
        }
    }

    #[test]
    fn test_validate_non_emptying_txn() {
        let reserve_balance = Balance::from(RESERVE_BALANCE);
        let latest_seq_num = SeqNum(1000);
        let txn_value = 1000;
        let block_seq_num = latest_seq_num + EXEC_DELAY - SeqNum(1);

        let tx = make_test_tx(50000, txn_value, 0, S1);
        let txs = vec![tx.clone()];
        let signer = tx.recover_signer().unwrap();
        let min_balance = compute_txn_max_gas_cost(&tx, BASE_FEE);

        let mut account_balances: BTreeMap<&Address, AccountBalanceState> = BTreeMap::new();
        account_balances.insert(
            &signer,
            AccountBalanceState {
                balance: min_balance,
                remaining_reserve_balance: min_balance,
                block_seqnum_of_latest_txn: latest_seq_num,
                max_reserve_balance: reserve_balance,
                is_delegated: false,
            },
        );

        let validator = EthBlockPolicyBlockValidator::new(
            block_seq_num,
            EXEC_DELAY,
            BASE_FEE,
            &MockChainRevision::DEFAULT,
            &MonadExecutionRevision::LATEST,
        )
        .unwrap();

        for txn in txs.iter() {
            assert!(validator
                .try_add_transaction(&mut account_balances, txn)
                .is_ok());
        }

        let mut account_balances: BTreeMap<&Address, AccountBalanceState> = BTreeMap::new();
        account_balances.insert(
            &signer,
            AccountBalanceState {
                balance: min_balance,
                remaining_reserve_balance: min_balance - Balance::from(1),
                block_seqnum_of_latest_txn: latest_seq_num,
                max_reserve_balance: reserve_balance,
                is_delegated: false,
            },
        );

        let validator = EthBlockPolicyBlockValidator::new(
            block_seq_num,
            EXEC_DELAY,
            BASE_FEE,
            &MockChainRevision::DEFAULT,
            &MonadExecutionRevision::LATEST,
        )
        .unwrap();

        for txn in txs.iter() {
            assert!(
                validator.try_add_transaction(&mut account_balances, txn)
                    == Err(BlockPolicyError::BlockPolicyBlockValidatorError(
                        BlockPolicyBlockValidatorError::InsufficientReserveBalance
                    ))
            );
        }
    }

    #[test]
    fn test_missing_balance() {
        let reserve_balance = Balance::from(RESERVE_BALANCE);
        let latest_seq_num = SeqNum(1000);
        let txn_value = 1000;
        let block_seq_num = latest_seq_num + EXEC_DELAY;

        let tx = make_test_tx(50000, txn_value, 0, S1);
        let txs = vec![tx.clone()];
        let min_balance = compute_txn_max_gas_cost(&tx, BASE_FEE);

        let address = Address(FixedBytes([0x11; 20]));

        let mut account_balances: BTreeMap<&Address, AccountBalanceState> = BTreeMap::new();
        account_balances.insert(
            &address,
            AccountBalanceState {
                balance: min_balance,
                remaining_reserve_balance: min_balance,
                block_seqnum_of_latest_txn: latest_seq_num,
                max_reserve_balance: reserve_balance,
                is_delegated: false,
            },
        );

        let validator = EthBlockPolicyBlockValidator::new(
            block_seq_num,
            EXEC_DELAY,
            BASE_FEE,
            &MockChainRevision::DEFAULT,
            &MonadExecutionRevision::LATEST,
        )
        .unwrap();

        for txn in txs.iter() {
            assert!(
                validator.try_add_transaction(&mut account_balances, txn)
                    == Err(BlockPolicyError::BlockPolicyBlockValidatorError(
                        BlockPolicyBlockValidatorError::AccountBalanceMissing
                    ))
            );
        }
    }

    #[test]
    fn test_validator_inconsistency() {
        let reserve_balance = Balance::from(RESERVE_BALANCE);
        let latest_seq_num = SeqNum(1000);
        let txn_value = 1000;
        let block_seq_num = latest_seq_num + EXEC_DELAY;

        let tx = make_test_tx(50000, txn_value, 0, S1);
        let txs = vec![tx.clone()];
        let signer = tx.recover_signer().unwrap();
        let min_balance = compute_txn_max_value(&tx, BASE_FEE);

        // Empty reserve balance
        let mut account_balances: BTreeMap<&Address, AccountBalanceState> = BTreeMap::new();
        account_balances.insert(
            &signer,
            AccountBalanceState {
                balance: min_balance,
                remaining_reserve_balance: Balance::ZERO,
                block_seqnum_of_latest_txn: latest_seq_num,
                max_reserve_balance: reserve_balance,
                is_delegated: false,
            },
        );

        let validator = EthBlockPolicyBlockValidator::new(
            block_seq_num,
            EXEC_DELAY,
            BASE_FEE,
            &MockChainRevision::DEFAULT,
            &MonadExecutionRevision::LATEST,
        )
        .unwrap();

        for txn in txs.iter() {
            assert!(validator
                .try_add_transaction(&mut account_balances, txn)
                .is_ok());
        }

        // Overdraft
        let block_seq_num = latest_seq_num;
        let min_reserve = compute_txn_max_gas_cost(&tx, BASE_FEE);
        let mut account_balances: BTreeMap<&Address, AccountBalanceState> = BTreeMap::new();
        account_balances.insert(
            &signer,
            AccountBalanceState {
                balance: Balance::ZERO,
                remaining_reserve_balance: min_reserve,
                block_seqnum_of_latest_txn: latest_seq_num,
                max_reserve_balance: reserve_balance,
                is_delegated: false,
            },
        );

        let validator = EthBlockPolicyBlockValidator::new(
            block_seq_num,
            EXEC_DELAY,
            BASE_FEE,
            &MockChainRevision::DEFAULT,
            &MonadExecutionRevision::LATEST,
        )
        .unwrap();

        for txn in txs.iter() {
            assert!(validator
                .try_add_transaction(&mut account_balances, txn)
                .is_ok());
        }
    }

    #[test]
    fn test_validate_many_txn() {
        let reserve_balance = Balance::from(RESERVE_BALANCE);
        let latest_seq_num = SeqNum(1000);
        let txn_value = 1000;
        let block_seq_num = latest_seq_num + EXEC_DELAY;

        let tx1 = make_test_tx(50000, txn_value, 0, S1);
        let tx2 = make_test_tx(50000, txn_value * 2, 1, S1);
        let signer = tx1.recover_signer().unwrap();

        let txs = vec![tx1.clone(), tx2.clone()];
        let min_balance =
            compute_txn_max_value(&tx1, BASE_FEE) + compute_txn_max_gas_cost(&tx2, BASE_FEE);

        let mut account_balances: BTreeMap<&Address, AccountBalanceState> = BTreeMap::new();
        account_balances.insert(
            &signer,
            AccountBalanceState {
                balance: min_balance,
                remaining_reserve_balance: Balance::ZERO,
                block_seqnum_of_latest_txn: latest_seq_num,
                max_reserve_balance: reserve_balance,
                is_delegated: false,
            },
        );

        let validator = EthBlockPolicyBlockValidator::new(
            block_seq_num,
            EXEC_DELAY,
            BASE_FEE,
            &MockChainRevision::DEFAULT,
            &MonadExecutionRevision::LATEST,
        )
        .unwrap();

        for txn in txs.iter() {
            assert!(validator
                .try_add_transaction(&mut account_balances, txn)
                .is_ok());
        }

        let min_reserve =
            compute_txn_max_gas_cost(&tx1, BASE_FEE) + compute_txn_max_gas_cost(&tx2, BASE_FEE);

        let mut account_balances: BTreeMap<&Address, AccountBalanceState> = BTreeMap::new();
        account_balances.insert(
            &signer,
            AccountBalanceState {
                balance: Balance::ZERO,
                remaining_reserve_balance: min_reserve,
                block_seqnum_of_latest_txn: latest_seq_num,
                max_reserve_balance: reserve_balance,
                is_delegated: false,
            },
        );

        let validator = EthBlockPolicyBlockValidator::new(
            latest_seq_num,
            EXEC_DELAY,
            BASE_FEE,
            &MockChainRevision::DEFAULT,
            &MonadExecutionRevision::LATEST,
        )
        .unwrap();

        for txn in txs.iter() {
            assert!(validator
                .try_add_transaction(&mut account_balances, txn)
                .is_ok());
        }
    }

    const RESERVE_FAIL: Result<(), BlockPolicyError> =
        Err(BlockPolicyError::BlockPolicyBlockValidatorError(
            BlockPolicyBlockValidatorError::InsufficientReserveBalance,
        ));

    const BALANCE_FAIL: Result<(), BlockPolicyError> =
        Err(BlockPolicyError::BlockPolicyBlockValidatorError(
            BlockPolicyBlockValidatorError::InsufficientBalance,
        ));

    #[rstest]
    #[case(Balance::from(100), Balance::from(10), Balance::from(10), SeqNum(3), 1_u128, 1_u64, Ok(()))]
    #[case(Balance::from(5), Balance::from(10), Balance::from(10), SeqNum(3), 2_u128, 2_u64, Ok(()))]
    #[case(Balance::from(5), Balance::from(5), Balance::from(5), SeqNum(3), 0_u128, 5_u64, Ok(()))]
    #[case(
        Balance::from(5),
        Balance::from(10),
        Balance::from(10),
        SeqNum(3),
        7_u128,
        2_u64,
        Ok(())
    )]
    #[case(
        Balance::from(100),
        Balance::from(1),
        Balance::from(1),
        SeqNum(2),
        3_u128,
        2_u64,
        RESERVE_FAIL
    )]
    // emptying txn case
    #[case(
        Balance::from(100),
        Balance::from(10),
        Balance::from(10),
        SeqNum(3),
        100_u128,
        100_u64,
        Ok(())
    )]
    fn test_txn_tfm(
        #[case] account_balance: Balance,
        #[case] reserve_balance: Balance,
        #[case] max_reserve_balance: Balance,
        #[case] block_seq_num: SeqNum,
        #[case] txn_value: u128,
        #[case] txn_gas_limit: u64,
        #[case] expect: Result<(), BlockPolicyError>,
    ) {
        let abs = AccountBalanceState {
            balance: account_balance,
            remaining_reserve_balance: reserve_balance,
            block_seqnum_of_latest_txn: SeqNum(0),
            max_reserve_balance,
            is_delegated: false,
        };

        let txn = make_test_eip1559_tx(txn_value, 0, txn_gas_limit, S1);
        let signer = txn.recover_signer().unwrap();

        let mut account_balances: BTreeMap<&Address, AccountBalanceState> = BTreeMap::new();
        account_balances.insert(&signer, abs);

        let validator = EthBlockPolicyBlockValidator::new(
            block_seq_num,
            EXEC_DELAY,
            BASE_FEE,
            &MockChainRevision::DEFAULT,
            &MonadExecutionRevision::LATEST,
        )
        .unwrap();

        assert_eq!(
            validator.try_add_transaction(&mut account_balances, &txn),
            expect
        );
    }

    #[rstest]
    #[case(
        Balance::from(100),
        Balance::from(5),
        Balance::from(10),
        vec![(4_u128, 2_u64), (4_u128, 2_u64), (4_u128, 2_u64)],
        vec![SeqNum(1), SeqNum(2), SeqNum(3)],
        vec![Ok(()), Ok(()), RESERVE_FAIL],
    )]
    #[case(
        Balance::from(100),
        Balance::from(6),
        Balance::from(10),
        vec![(4_u128, 2_u64), (4_u128, 2_u64), (4_u128, 2_u64)],
        vec![SeqNum(1), SeqNum(2), SeqNum(3)],
        vec![Ok(()), Ok(()), Ok(())],
    )]
    // attempt to empty, but other txns within k
    #[case(
        Balance::from(100),
        Balance::from(10),
        Balance::from(10),
        vec![(4_u128, 4_u64), (96_u128, 96_u64)],
        vec![SeqNum(1), SeqNum(3)],
        vec![Ok(()), RESERVE_FAIL],
    )]
    // successful attempt to empty because emptying is more than k
    #[case(
        Balance::from(100),
        Balance::from(10),
        Balance::from(10),
        vec![(4_u128, 4_u64), (96_u128, 0_u64)],
        vec![SeqNum(1), SeqNum(4)],
        vec![Ok(()), Ok(())],
    )]
    // single emptying txn
    #[case(
        Balance::from(100),
        Balance::from(10),
        Balance::from(10),
        vec![(100_u128, 4_u64)],
        vec![SeqNum(4)],
        vec![Ok(())],
    )]
    // Test transactions across multiple blocks with
    // enough cooling period (k gap between txns)
    // but reserve lower than fees
    #[case(
        Balance::from(100),
        Balance::from(1),
        Balance::from(1),
        vec![(3_u128, 2_u64), (3_u128, 2_u64)],
        vec![SeqNum(4), SeqNum(7)],
        vec![Ok(()), Ok(())],
    )]
    // Test with low reserve balance
    #[case(
        Balance::from(100),
        Balance::from(1),
        Balance::from(1),
        vec![(3_u128, 2_u64), (3_u128, 2_u64)],
        vec![SeqNum(4), SeqNum(4)],
        vec![Ok(()), RESERVE_FAIL],
    )]
    // Test inclusion but will revert in execution
    // first txn bring balance down to reserve (Ok)
    // second tries to spend all (fails)
    // third is allowed by consensus but expected to revert in execution
    // because it tries to dip into reserve
    #[case(
        Balance::from(1000),
        Balance::from(10),
        Balance::from(10),
        vec![(990_u128, 1_u64), (5_u128, 10_u64), (8_u128, 1_u64)],
        vec![SeqNum(4), SeqNum(5), SeqNum(6)],
        vec![Ok(()), RESERVE_FAIL, Ok(())],
    )]
    // Zero value transfer test
    // first txn is not potentially emptying
    #[case(
        Balance::from(20),
        Balance::from(10),
        Balance::from(10),
        vec![(0_u128, 5_u64), (0_u128, 5_u64), (0_u128, 5_u64)],
        vec![SeqNum(2), SeqNum(2), SeqNum(2)],
        vec![Ok(()), Ok(()), RESERVE_FAIL],
    )]
    // Reserve boundary test
    // first txn is not potentially emptying
    #[case(
        Balance::from(100),
        Balance::from(20),
        Balance::from(20),
        vec![(10_u128, 20_u64), (0_u128, 1_u64)],
        vec![SeqNum(2), SeqNum(2)],
        vec![Ok(()), RESERVE_FAIL],
    )]
    // Zero value transfer test
    // first txn is potentially emptying
    #[case(
        Balance::from(20),
        Balance::from(10),
        Balance::from(10),
        vec![(0_u128, 5_u64), (0_u128, 5_u64), (0_u128, 5_u64)],
        vec![SeqNum(3), SeqNum(3), SeqNum(3)],
        vec![Ok(()), Ok(()), Ok(())],
    )]
    // Reserve boundary test
    // first txn is potentially emptying
    #[case(
        Balance::from(100),
        Balance::from(20),
        Balance::from(20),
        vec![(10_u128, 20_u64), (0_u128, 1_u64)],
        vec![SeqNum(3), SeqNum(3)],
        vec![Ok(()), Ok(())],
    )]
    // Reserve boundary test with many txns
    // first txn is potentially emptying
    // the last txn in the list exceeds the reserve balance
    #[case(
        Balance::from(100),
        Balance::from(20),
        Balance::from(20),
        vec![(10_u128, 20_u64), (0_u128, 1_u64), (0_u128, 18_u64), (0_u128, 1_u64), (0_u128, 1_u64)],
        vec![SeqNum(3), SeqNum(3), SeqNum(3), SeqNum(3), SeqNum(3)],
        vec![Ok(()), Ok(()), Ok(()), Ok(()), RESERVE_FAIL],
    )]
    fn test_multi_txn_tfm(
        #[case] account_balance: Balance,
        #[case] reserve_balance: Balance,
        #[case] max_reserve_balance: Balance,
        #[case] txns: Vec<(u128, u64)>, // txn (value, gas_limit)
        #[case] txn_block_num: Vec<SeqNum>,
        #[case] expected: Vec<Result<(), BlockPolicyError>>,
    ) {
        multi_txn_tfm_helper(
            false,
            account_balance,
            reserve_balance,
            max_reserve_balance,
            txns,
            txn_block_num,
            expected,
        )
    }

    #[rstest]
    // attempt to exceed reserve after k blocks but will fail
    // because delegated
    #[case(
        Balance::from(100),
        Balance::from(10),
        Balance::from(10),
        vec![(20_u128, 15_u64)],
        vec![SeqNum(3)],
        vec![RESERVE_FAIL],
    )]
    // Reserve boundary test
    #[case(
        Balance::from(100),
        Balance::from(20),
        Balance::from(20),
        vec![(10_u128, 20_u64), (0_u128, 1_u64)],
        vec![SeqNum(3), SeqNum(3)],
        vec![Ok(()), RESERVE_FAIL],
    )]
    fn test_try_add_delegated_txns(
        #[case] account_balance: Balance,
        #[case] reserve_balance: Balance,
        #[case] max_reserve_balance: Balance,
        #[case] txns: Vec<(u128, u64)>, // txn (value, gas_limit)
        #[case] txn_block_num: Vec<SeqNum>,
        #[case] expected: Vec<Result<(), BlockPolicyError>>,
    ) {
        multi_txn_tfm_helper(
            true,
            account_balance,
            reserve_balance,
            max_reserve_balance,
            txns,
            txn_block_num,
            expected,
        )
    }

    fn multi_txn_tfm_helper(
        is_delegated: bool,
        account_balance: Balance,
        reserve_balance: Balance,
        max_reserve_balance: Balance,
        txns: Vec<(u128, u64)>, // txn (value, gas_limit)
        txn_block_num: Vec<SeqNum>,
        expected: Vec<Result<(), BlockPolicyError>>,
    ) {
        assert_eq!(txns.len(), expected.len());
        assert_eq!(txns.len(), txn_block_num.len());

        let abs = AccountBalanceState {
            balance: account_balance,
            remaining_reserve_balance: reserve_balance,
            block_seqnum_of_latest_txn: SeqNum(0),
            max_reserve_balance,
            is_delegated,
        };

        let txns = txns
            .iter()
            .enumerate()
            .map(|(nonce, (value, gas_limit))| {
                make_test_eip1559_tx(*value, nonce as u64, *gas_limit, S1)
            })
            .collect_vec();
        let signer = txns[0].recover_signer().unwrap();

        let mut account_balances: BTreeMap<&Address, AccountBalanceState> = BTreeMap::new();
        account_balances.insert(&signer, abs);

        for ((tx, expect), seqnum) in txns.into_iter().zip(expected).zip(txn_block_num) {
            check_txn_helper(seqnum, &mut account_balances, &tx, expect);
        }
    }

    fn check_txn_helper(
        block_seq_num: SeqNum,
        account_balances: &mut BTreeMap<&Address, AccountBalanceState>,
        txn: &Recovered<TxEnvelope>,
        expect: Result<(), BlockPolicyError>,
    ) {
        let validator = EthBlockPolicyBlockValidator::new(
            block_seq_num,
            EXEC_DELAY,
            BASE_FEE,
            &MockChainRevision::DEFAULT,
            &MonadExecutionRevision::LATEST,
        )
        .unwrap();

        assert_eq!(
            validator.try_add_transaction(account_balances, txn),
            expect,
            "txn nonce {}",
            txn.nonce()
        );
    }

    fn make_test_eip1559_tx(
        value: u128,
        nonce: u64,
        gas_limit: u64,
        signer: FixedBytes<32>,
    ) -> Recovered<TxEnvelope> {
        recover_tx(make_eip1559_tx_with_value(
            signer, value, 1_u128, 0, gas_limit, nonce, 0,
        ))
    }

    fn make_txn_fees(
        first_txn_value: u64,
        first_txn_gas: u64,
        max_gas_cost: u64,
        is_delegated: bool,
    ) -> TxnFee {
        TxnFee {
            first_txn_value: Balance::from(first_txn_value),
            first_txn_gas: Balance::from(first_txn_gas),
            max_gas_cost: Balance::from(max_gas_cost),
            max_txn_cost: Balance::ZERO,
            is_delegated,
        }
    }

    fn apply_block_fees_helper(
        block_seq_num: SeqNum,
        account_balance: &mut AccountBalanceState,
        fees: &TxnFee,
        eth_address: &Address,
        expected_remaining_reserve: Balance,
        expected_is_delegated: bool,
        expect: Result<(), BlockPolicyError>,
    ) {
        let validator = EthBlockPolicyBlockValidator::new(
            block_seq_num,
            EXEC_DELAY,
            BASE_FEE,
            &MockChainRevision::DEFAULT,
            &MonadExecutionRevision::LATEST,
        )
        .unwrap();

        assert_eq!(
            validator.try_apply_block_fees(account_balance, fees, eth_address),
            expect,
        );
        assert_eq!(
            account_balance.remaining_reserve_balance,
            expected_remaining_reserve
        );
        assert_eq!(account_balance.is_delegated, expected_is_delegated);
    }

    #[rstest]
    #[case( // Has emptying txn, insufficient balance
        Balance::from(100),
        Balance::from(10),
        Balance::from(10),
        SeqNum(1),
        vec![(1001, 1, 100)], // value is not checked
        vec![SeqNum(4)],
        vec![Balance::ZERO],
        vec![RESERVE_FAIL],
    )]
    #[case( // Has emptying txn, insufficient reserve
        Balance::from(100),
        Balance::from(10),
        Balance::from(10),
        SeqNum(1),
        vec![(100, 1, 100)],
        vec![SeqNum(4)],
        vec![Balance::ZERO],
        vec![RESERVE_FAIL],
    )]
    #[case( // Has emptying txn, insufficient reserve
        Balance::from(100),
        Balance::from(10),
        Balance::from(10),
        SeqNum(1),
        vec![(90, 1, 4), (5, 1, 5)],
        vec![SeqNum(4), SeqNum(5)],
        vec![Balance::from(5), Balance::from(5)],
        vec![Ok(()), RESERVE_FAIL],
    )]
    #[case( // Has emptying txn, pass 
        Balance::from(100),
        Balance::from(10),
        Balance::from(10),
        SeqNum(1),
        vec![(90, 1, 4), (5, 1, 4)],
        vec![SeqNum(4), SeqNum(5)],
        vec![Balance::from(5), Balance::from(0)],
        vec![Ok(()), Ok(())],
    )]
    #[case( // reserve balance fail
        Balance::from(100),
        Balance::from(10),
        Balance::from(10),
        SeqNum(0),
        vec![(50, 1, 9), (0, 0, 0), (500, 1, 1)],
        vec![SeqNum(1), SeqNum(2), SeqNum(3)],
        vec![Balance::from(0), Balance::from(0)],
        vec![Ok(()), Ok(()), RESERVE_FAIL],
    )]
    fn test_try_apply_block_fees(
        #[case] account_balance: Balance,
        #[case] reserve_balance: Balance,
        #[case] max_reserve_balance: Balance,
        #[case] block_seqnum_of_latest_txn: SeqNum,
        #[case] blk_fees: Vec<(u64, u64, u64)>, // (first_txn_value, first_txn_gas, max_gas_cost)
        #[case] txn_block_num: Vec<SeqNum>,
        #[case] expected_remaining_reserve: Vec<Balance>,
        #[case] expected: Vec<Result<(), BlockPolicyError>>,
    ) {
        assert_eq!(blk_fees.len(), expected.len());
        assert_eq!(blk_fees.len(), txn_block_num.len());

        let address = Address(FixedBytes([0x11; 20]));

        let mut account_balance = AccountBalanceState {
            balance: account_balance,
            remaining_reserve_balance: reserve_balance,
            block_seqnum_of_latest_txn,
            max_reserve_balance,
            is_delegated: false,
        };

        let blk_fees = blk_fees
            .into_iter()
            .map(|x| make_txn_fees(x.0, x.1, x.2, false))
            .collect_vec();

        for (((fees, expect), seqnum), expected_remaining_reserve) in blk_fees
            .into_iter()
            .zip(expected)
            .zip(txn_block_num)
            .zip(expected_remaining_reserve)
        {
            apply_block_fees_helper(
                seqnum,
                &mut account_balance,
                &fees,
                &address,
                expected_remaining_reserve,
                false,
                expect,
            );
        }
    }

    #[test]
    fn test_resolve_authorizations_in_extending() {
        let block_policy = EthBlockPolicy::<
            SignatureType,
            SignatureCollectionType,
            ChainConfigType,
            ChainRevisionType,
        >::new(GENESIS_SEQ_NUM, EXEC_DELAY.0);

        let state_backend = NopStateBackend::default();

        let addresses = [secret_to_eth_address(S2)];

        // Valid authorization nonce increments nonce
        {
            let (s2, s2_nonce) = block_policy
                .get_account_base_nonces(
                    SeqNum(1),
                    &state_backend,
                    &vec![&make_test_block(
                        Round(1),
                        SeqNum(1),
                        vec![recover_tx(make_eip7702_tx(
                            S1,
                            BASE_FEE as u128,
                            0,
                            100_000,
                            0,
                            vec![make_signed_authorization(S2, secret_to_eth_address(S1), 0)],
                            0,
                        ))],
                    )],
                    addresses.iter(),
                )
                .unwrap()
                .pop_first()
                .unwrap();

            assert_eq!(s2, &secret_to_eth_address(S2));
            assert_eq!(s2_nonce, 1);
        }

        // Bad nonce invalidates authorization
        {
            let (s2, s2_nonce) = block_policy
                .get_account_base_nonces(
                    SeqNum(1),
                    &state_backend,
                    &vec![&make_test_block(
                        Round(1),
                        SeqNum(1),
                        vec![recover_tx(make_eip7702_tx(
                            S1,
                            BASE_FEE as u128,
                            0,
                            100_000,
                            0,
                            vec![make_signed_authorization(S2, secret_to_eth_address(S1), 1)],
                            0,
                        ))],
                    )],
                    addresses.iter(),
                )
                .unwrap()
                .pop_first()
                .unwrap();

            assert_eq!(s2, &secret_to_eth_address(S2));
            assert_eq!(s2_nonce, 0);
        }

        // Only resolve nonce in the last execution_delay blocks
        {
            let (s2, s2_nonce) = block_policy
                .get_account_base_nonces(
                    SeqNum(10),
                    &state_backend,
                    &vec![
                        // nonces beyond execution_delay should not be taken into account
                        &make_test_block(
                            Round(1),
                            SeqNum(7),
                            vec![recover_tx(make_eip7702_tx(
                                S1,
                                BASE_FEE as u128,
                                0,
                                100_000,
                                0,
                                vec![make_signed_authorization(S2, secret_to_eth_address(S1), 0)],
                                0,
                            ))],
                        ),
                        &make_test_block(Round(1), SeqNum(8), vec![]),
                        &make_test_block(Round(1), SeqNum(9), vec![]),
                    ],
                    addresses.iter(),
                )
                .unwrap()
                .pop_first()
                .unwrap();

            assert_eq!(s2, &secret_to_eth_address(S2));
            assert_eq!(s2_nonce, 0);
        }
    }

    #[rstest]
    #[case( // Has emptying txn, no auth in fly
        Balance::from(1),
        Balance::from(10),
        SeqNum(1),
        false,
        vec![(11, 1, 1, false)],
        vec![SeqNum(4)],
        vec![Balance::from(0_u64)],
        vec![false],
        vec![RESERVE_FAIL],
    )]
    #[case( // Has emptying txn, auth in fly
        Balance::from(1),
        Balance::from(10),
        SeqNum(1),
        true,
        vec![(11, 1, 1, false)],
        vec![SeqNum(4)],
        vec![Balance::from(8_u64)],
        vec![true],
        vec![Ok(())],
    )]
    #[case( // delegated in initial state, auth
        Balance::from(100),
        Balance::from(10),
        SeqNum(1),
        true,
        vec![(11, 1, 1, true)],
        vec![SeqNum(4)],
        vec![Balance::from(8_u64)],
        vec![true],
        vec![Ok(())],
    )]
    #[case( // delegated in initial state, no auth
        Balance::from(100),
        Balance::from(10),
        SeqNum(1),
        true,
        vec![(11, 1, 1, false)],
        vec![SeqNum(4)],
        vec![Balance::from(8_u64)],
        vec![true],
        vec![Ok(())],
    )]
    #[case( // delegated in initial state, auth
        Balance::from(100),
        Balance::from(10),
        SeqNum(1),
        false,
        vec![(11, 1, 10, true), (11, 1, 1, false)],
        vec![SeqNum(4), SeqNum(5)],
        vec![Balance::from(78_u64), Balance::from(76_u64)],
        vec![true, true],
        vec![Ok(()), Ok(())],
    )]
    #[case( // delegated in initial state, auth
        Balance::from(1),
        Balance::from(10),
        SeqNum(1),
        false,
        vec![(11, 1, 2, true), (11, 1, 1, false), (4, 2, 0, false)],
        vec![SeqNum(1), SeqNum(2), SeqNum(5)],
        vec![Balance::from(7_u64), Balance::from(5_u64), Balance::from(3_u64)],
        vec![true, true, true],
        vec![Ok(()), Ok(()), Ok(())],
    )]
    #[case( // delegated in initial state, auth
        Balance::from(1),
        Balance::from(10),
        SeqNum(1),
        false,
        vec![(11, 1, 2, false), (11, 1, 1, false), (4, 2, 0, false)],
        vec![SeqNum(1), SeqNum(2), SeqNum(5)],
        vec![Balance::from(7_u64), Balance::from(5_u64), Balance::from(5_u64)],
        vec![false, false, false],
        vec![Ok(()), Ok(()), BALANCE_FAIL],
    )]

    fn test_compute_delegation(
        #[case] account_balance: Balance,
        #[case] reserve_balance: Balance,
        #[case] block_seqnum_of_latest_txn: SeqNum,
        #[case] is_delegated: bool,
        #[case] blk_fees: Vec<(u64, u64, u64, bool)>, // (first_txn_value, first_txn_gas, max_gas_cost, is_delegated)
        #[case] txn_block_num: Vec<SeqNum>,
        #[case] expected_remaining_reserve: Vec<Balance>,
        #[case] expected_is_delegated: Vec<bool>,
        #[case] expected: Vec<Result<(), BlockPolicyError>>,
    ) {
        assert_eq!(blk_fees.len(), expected.len());
        assert_eq!(blk_fees.len(), txn_block_num.len());

        let address = Address(FixedBytes([0x11; 20]));

        let mut account_balance = AccountBalanceState {
            balance: account_balance,
            remaining_reserve_balance: reserve_balance,
            max_reserve_balance: Balance::ZERO, // unused
            block_seqnum_of_latest_txn,
            is_delegated,
        };

        let blk_fees = blk_fees
            .into_iter()
            .map(|x| make_txn_fees(x.0, x.1, x.2, x.3))
            .collect_vec();

        for ((((fees, expect), seqnum), expected_remaining_reserve), expected_is_delegated) in
            blk_fees
                .into_iter()
                .zip(expected)
                .zip(txn_block_num)
                .zip(expected_remaining_reserve)
                .zip(expected_is_delegated)
        {
            apply_block_fees_helper(
                seqnum,
                &mut account_balance,
                &fees,
                &address,
                expected_remaining_reserve,
                expected_is_delegated,
                expect,
            );
        }
    }
}
