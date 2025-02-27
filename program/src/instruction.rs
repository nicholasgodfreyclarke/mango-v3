use crate::matching::{OrderType, Side};
use crate::state::MAX_PAIRS;
use crate::state::{AssetType, INFO_LEN};
use arrayref::{array_ref, array_refs};
use fixed::types::I80F48;
use num_enum::TryFromPrimitive;
use serde::{Deserialize, Serialize};
use solana_program::instruction::{AccountMeta, Instruction};
use solana_program::program_error::ProgramError;
use solana_program::pubkey::Pubkey;
use std::convert::{TryFrom, TryInto};
use std::num::NonZeroU64;

#[repr(C)]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum MangoInstruction {
    /// Initialize a group of lending pools that can be cross margined
    ///
    /// Accounts expected by this instruction (12):
    ///
    /// 0. `[writable]` mango_group_ai
    /// 1. `[]` signer_ai
    /// 2. `[]` admin_ai
    /// 3. `[]` quote_mint_ai
    /// 4. `[]` quote_vault_ai
    /// 5. `[writable]` quote_node_bank_ai
    /// 6. `[writable]` quote_root_bank_ai
    /// 7. `[]` dao_vault_ai - aka insurance fund
    /// 8. `[]` msrm_vault_ai - msrm deposits for fee discounts; can be Pubkey::default()
    /// 9. `[]` fees_vault_ai - vault owned by Mango DAO token governance to receive fees
    /// 10. `[writable]` mango_cache_ai - Account to cache prices, root banks, and perp markets
    /// 11. `[]` dex_prog_ai
    InitMangoGroup {
        signer_nonce: u64,
        valid_interval: u64,
        quote_optimal_util: I80F48,
        quote_optimal_rate: I80F48,
        quote_max_rate: I80F48,
    },

    /// Initialize a mango account for a user
    ///
    /// Accounts expected by this instruction (4):
    ///
    /// 0. `[]` mango_group_ai - MangoGroup that this mango account is for
    /// 1. `[writable]` mango_account_ai - the mango account data
    /// 2. `[signer]` owner_ai - Solana account of owner of the mango account
    /// 3. `[]` rent_ai - Rent sysvar account
    InitMangoAccount,

    /// Deposit funds into mango account
    ///
    /// Accounts expected by this instruction (8):
    ///
    /// 0. `[]` mango_group_ai - MangoGroup that this mango account is for
    /// 1. `[writable]` mango_account_ai - the mango account for this user
    /// 2. `[signer]` owner_ai - Solana account of owner of the mango account
    /// 3. `[]` mango_cache_ai - MangoCache
    /// 4. `[]` root_bank_ai - RootBank owned by MangoGroup
    /// 5. `[writable]` node_bank_ai - NodeBank owned by RootBank
    /// 6. `[writable]` vault_ai - TokenAccount owned by MangoGroup
    /// 7. `[]` token_prog_ai - acc pointed to by SPL token program id
    /// 8. `[writable]` owner_token_account_ai - TokenAccount owned by user which will be sending the funds
    Deposit {
        quantity: u64,
    },

    /// Withdraw funds that were deposited earlier.
    ///
    /// Accounts expected by this instruction (10):
    ///
    /// 0. `[read]` mango_group_ai,   -
    /// 1. `[write]` mango_account_ai, -
    /// 2. `[read]` owner_ai,         -
    /// 3. `[read]` mango_cache_ai,   -
    /// 4. `[read]` root_bank_ai,     -
    /// 5. `[write]` node_bank_ai,     -
    /// 6. `[write]` vault_ai,         -
    /// 7. `[write]` token_account_ai, -
    /// 8. `[read]` signer_ai,        -
    /// 9. `[read]` token_prog_ai,    -
    /// 10. `[read]` clock_ai,         -
    /// 11..+ `[]` open_orders_accs - open orders for each of the spot market
    Withdraw {
        quantity: u64,
        allow_borrow: bool,
    },

    /// Add a token to a mango group
    ///
    /// Accounts expected by this instruction (8):
    ///
    /// 0. `[writable]` mango_group_ai
    /// 1  `[]` oracle_ai
    /// 2. `[]` spot_market_ai
    /// 3. `[]` dex_program_ai
    /// 4. `[]` mint_ai
    /// 5. `[writable]` node_bank_ai
    /// 6. `[]` vault_ai
    /// 7. `[writable]` root_bank_ai
    /// 8. `[signer]` admin_ai
    AddSpotMarket {
        maint_leverage: I80F48,
        init_leverage: I80F48,
        liquidation_fee: I80F48,
        optimal_util: I80F48,
        optimal_rate: I80F48,
        max_rate: I80F48,
    },

    /// DEPRECATED
    AddToBasket {
        market_index: usize,
    },

    /// DEPRECATED - use Withdraw with allow_borrow = true
    Borrow {
        quantity: u64,
    },

    /// Cache prices
    ///
    /// Accounts expected: 3 + Oracles
    /// 0. `[]` mango_group_ai -
    /// 1. `[writable]` mango_cache_ai -
    /// 2+... `[]` oracle_ais - flux aggregator feed accounts
    CachePrices,

    /// Cache root banks
    ///
    /// Accounts expected: 2 + Root Banks
    /// 0. `[]` mango_group_ai
    /// 1. `[writable]` mango_cache_ai
    CacheRootBanks,

    /// Place an order on the Serum Dex using Mango account
    ///
    /// Accounts expected by this instruction (23 + MAX_PAIRS):
    /// 0. `[]` mango_group_ai - MangoGroup
    /// 1. `[writable]` mango_account_ai - the MangoAccount of owner
    /// 2. `[signer]` owner_ai - owner of MangoAccount
    /// 3. `[]` mango_cache_ai - MangoCache for this MangoGroup
    /// 4. `[]` dex_prog_ai - serum dex program id
    /// 5. `[writable]` spot_market_ai - serum dex MarketState account
    /// 6. `[writable]` bids_ai - bids account for serum dex market
    /// 7. `[writable]` asks_ai - asks account for serum dex market
    /// 8. `[writable]` dex_request_queue_ai - request queue for serum dex market
    /// 9. `[writable]` dex_event_queue_ai - event queue for serum dex market
    /// 10. `[writable]` dex_base_ai - base currency serum dex market vault
    /// 11. `[writable]` dex_quote_ai - quote currency serum dex market vault
    /// 12. `[]` base_root_bank_ai - root bank of base currency
    /// 13. `[writable]` base_node_bank_ai - node bank of base currency
    /// 14. `[writable]` base_vault_ai - vault of the basenode bank
    /// 15. `[]` quote_root_bank_ai - root bank of quote currency
    /// 16. `[writable]` quote_node_bank_ai - node bank of quote currency
    /// 17. `[writable]` quote_vault_ai - vault of the quote node bank
    /// 18. `[]` token_prog_ai - SPL token program id
    /// 19. `[]` signer_ai - signer key for this MangoGroup
    /// 20. `[]` rent_ai - rent sysvar var
    /// 21. `[]` dex_signer_key - signer for serum dex
    /// 22. `[]` msrm_or_srm_vault_ai - the msrm or srm vault in this MangoGroup. Can be zero key
    /// 23+ `[writable]` open_orders_ais - An array of MAX_PAIRS. Only OpenOrders of current market
    ///         index needs to be writable. Only OpenOrders in_margin_basket needs to be correct;
    ///         remaining open orders can just be Pubkey::default() (the zero key)
    PlaceSpotOrder {
        order: serum_dex::instruction::NewOrderInstructionV3,
    },

    /// Add oracle
    ///
    /// Accounts expected: 3
    /// 0. `[writable]` mango_group_ai - MangoGroup
    /// 1. `[writable]` oracle_ai - oracle
    /// 2. `[signer]` admin_ai - admin
    AddOracle, // = 10

    /// Add a perp market to a mango group
    ///
    /// Accounts expected by this instruction (7):
    ///
    /// 0. `[writable]` mango_group_ai
    /// 1. `[]` oracle_ai
    /// 2. `[writable]` perp_market_ai
    /// 3. `[writable]` event_queue_ai
    /// 4. `[writable]` bids_ai
    /// 5. `[writable]` asks_ai
    /// 6. `[]` mngo_vault_ai - the vault from which liquidity incentives will be paid out for this market
    /// 7. `[signer]` admin_ai
    AddPerpMarket {
        maint_leverage: I80F48,
        init_leverage: I80F48,
        liquidation_fee: I80F48,
        maker_fee: I80F48,
        taker_fee: I80F48,
        base_lot_size: i64,
        quote_lot_size: i64,
        /// Starting rate for liquidity mining
        rate: I80F48,
        /// depth liquidity mining works for
        max_depth_bps: I80F48,
        /// target length in seconds of one period
        target_period_length: u64,
        /// amount MNGO rewarded per period
        mngo_per_period: u64,
    },

    /// Place an order on a perp market
    /// Accounts expected by this instruction (8):
    /// 0. `[]` mango_group_ai - MangoGroup
    /// 1. `[writable]` mango_account_ai - the MangoAccount of owner
    /// 2. `[signer]` owner_ai - owner of MangoAccount
    /// 3. `[]` mango_cache_ai - MangoCache for this MangoGroup
    /// 4. `[writable]` perp_market_ai
    /// 5. `[writable]` bids_ai - bids account for this PerpMarket
    /// 6. `[writable]` asks_ai - asks account for this PerpMarket
    /// 7. `[writable]` event_queue_ai - EventQueue for this PerpMarket
    PlacePerpOrder {
        price: i64,
        quantity: i64,
        client_order_id: u64,
        side: Side,
        /// Can be 0 -> LIMIT, 1 -> IOC, 2 -> PostOnly
        order_type: OrderType,
    },

    CancelPerpOrderByClientId {
        client_order_id: u64,
        invalid_id_ok: bool,
    },

    CancelPerpOrder {
        order_id: i128,
        invalid_id_ok: bool,
    },

    ConsumeEvents {
        limit: usize,
    },

    /// Cache perp markets
    ///
    /// Accounts expected: 2 + Perp Markets
    /// 0. `[]` mango_group_ai
    /// 1. `[writable]` mango_cache_ai
    CachePerpMarkets,

    /// Update funding related variables
    UpdateFunding,

    /// Can only be used on a stub oracle in devnet
    SetOracle {
        price: I80F48,
    },

    /// Settle all funds from serum dex open orders
    ///
    /// Accounts expected by this instruction (18):
    ///
    /// 0. `[]` mango_group_ai - MangoGroup that this mango account is for
    /// 1. `[]` mango_cache_ai - MangoCache for this MangoGroup
    /// 2. `[signer]` owner_ai - MangoAccount owner
    /// 3. `[writable]` mango_account_ai - MangoAccount
    /// 4. `[]` dex_prog_ai - program id of serum dex
    /// 5.  `[writable]` spot_market_ai - dex MarketState account
    /// 6.  `[writable]` open_orders_ai - open orders for this market for this MangoAccount
    /// 7. `[]` signer_ai - MangoGroup signer key
    /// 8. `[writable]` dex_base_ai - base vault for dex MarketState
    /// 9. `[writable]` dex_quote_ai - quote vault for dex MarketState
    /// 10. `[]` base_root_bank_ai - MangoGroup base vault acc
    /// 11. `[writable]` base_node_bank_ai - MangoGroup quote vault acc
    /// 12. `[]` quote_root_bank_ai - MangoGroup quote vault acc
    /// 13. `[writable]` quote_node_bank_ai - MangoGroup quote vault acc
    /// 14. `[writable]` base_vault_ai - MangoGroup base vault acc
    /// 15. `[writable]` quote_vault_ai - MangoGroup quote vault acc
    /// 16. `[]` dex_signer_ai - dex Market signer account
    /// 17. `[]` spl token program
    SettleFunds,

    /// Cancel an order using dex instruction
    ///
    /// Accounts expected by this instruction ():
    ///
    CancelSpotOrder {
        // 20
        order: serum_dex::instruction::CancelOrderInstructionV2,
    },

    /// Update a root bank's indexes by providing all it's node banks
    ///
    /// Accounts expected: 2 + Node Banks
    /// 0. `[]` mango_group_ai - MangoGroup
    /// 1. `[]` root_bank_ai - RootBank
    /// 2+... `[]` node_bank_ais - NodeBanks
    UpdateRootBank,

    /// Take two MangoAccounts and settle profits and losses between them for a perp market
    ///
    /// Accounts expected (6):
    SettlePnl {
        market_index: usize,
    },

    /// DEPRECATED - no longer makes sense
    /// Use this token's position and deposit to reduce borrows
    ///
    /// Accounts expected by this instruction (5):
    SettleBorrow {
        token_index: usize,
        quantity: u64,
    },

    /// Force cancellation of open orders for a user being liquidated
    ///
    /// Accounts expected: 19 + Liqee open orders accounts (MAX_PAIRS)
    /// 0. `[]` mango_group_ai - MangoGroup
    /// 1. `[]` mango_cache_ai - MangoCache
    /// 2. `[writable]` liqee_mango_account_ai - MangoAccount
    /// 3. `[]` base_root_bank_ai - RootBank
    /// 4. `[writable]` base_node_bank_ai - NodeBank
    /// 5. `[writable]` base_vault_ai - MangoGroup base vault acc
    /// 6. `[]` quote_root_bank_ai - RootBank
    /// 7. `[writable]` quote_node_bank_ai - NodeBank
    /// 8. `[writable]` quote_vault_ai - MangoGroup quote vault acc
    /// 9. `[writable]` spot_market_ai - SpotMarket
    /// 10. `[writable]` bids_ai - SpotMarket bids acc
    /// 11. `[writable]` asks_ai - SpotMarket asks acc
    /// 12. `[signer]` signer_ai - Signer
    /// 13. `[writable]` dex_event_queue_ai - Market event queue acc
    /// 14. `[writable]` dex_base_ai -
    /// 15. `[writable]` dex_quote_ai -
    /// 16. `[]` dex_signer_ai -
    /// 17. `[]` dex_prog_ai - Dex Program acc
    /// 18. `[]` token_prog_ai - Token Program acc
    /// 19+... `[]` liqee_open_orders_ais - Liqee open orders accs
    ForceCancelSpotOrders {
        limit: u8,
    },

    /// Force cancellation of open orders for a user being liquidated
    ///
    /// Accounts expected: 6 + Liqee open orders accounts (MAX_PAIRS)
    /// 0. `[]` mango_group_ai - MangoGroup
    /// 1. `[]` mango_cache_ai - MangoCache
    /// 2. `[]` perp_market_ai - PerpMarket
    /// 3. `[writable]` bids_ai - Bids acc
    /// 4. `[writable]` asks_ai - Asks acc
    /// 5. `[writable]` liqee_mango_account_ai - Liqee MangoAccount
    /// 6+... `[]` liqor_open_orders_ais - Liqee open orders accs
    ForceCancelPerpOrders {
        limit: u8,
    },

    /// Liquidator takes some of borrows at token at `liab_index` and receives some deposits from
    /// the token at `asset_index`
    ///
    /// Accounts expected: 9 + Liqee open orders accounts (MAX_PAIRS) + Liqor open orders accounts (MAX_PAIRS)
    /// 0. `[]` mango_group_ai - MangoGroup
    /// 1. `[]` mango_cache_ai - MangoCache
    /// 2. `[writable]` liqee_mango_account_ai - MangoAccount
    /// 3. `[writable]` liqor_mango_account_ai - MangoAccount
    /// 4. `[signer]` liqor_ai - Liqor Account
    /// 5. `[]` asset_root_bank_ai - RootBank
    /// 6. `[writable]` asset_node_bank_ai - NodeBank
    /// 7. `[]` liab_root_bank_ai - RootBank
    /// 8. `[writable]` liab_node_bank_ai - NodeBank
    /// 9+... `[]` liqee_open_orders_ais - Liqee open orders accs
    /// 9+MAX_PAIRS... `[]` liqor_open_orders_ais - Liqor open orders accs
    LiquidateTokenAndToken {
        max_liab_transfer: I80F48,
    },

    /// Swap tokens for perp quote position if only and only if the base position in that market is 0
    ///
    /// Accounts expected: 7 + Liqee open orders accounts (MAX_PAIRS) + Liqor open orders accounts (MAX_PAIRS)
    /// 0. `[]` mango_group_ai - MangoGroup
    /// 1. `[]` mango_cache_ai - MangoCache
    /// 2. `[writable]` liqee_mango_account_ai - MangoAccount
    /// 3. `[writable]` liqor_mango_account_ai - MangoAccount
    /// 4. `[signer]` liqor_ai - Liqor Account
    /// 5. `[]` root_bank_ai - RootBank
    /// 6. `[writable]` node_bank_ai - NodeBank
    /// 7+... `[]` liqee_open_orders_ais - Liqee open orders accs
    /// 7+MAX_PAIRS... `[]` liqor_open_orders_ais - Liqor open orders accs
    LiquidateTokenAndPerp {
        asset_type: AssetType,
        asset_index: usize,
        liab_type: AssetType,
        liab_index: usize,
        max_liab_transfer: I80F48,
    },

    /// Reduce some of the base position in exchange for quote position in this market
    ///
    /// Accounts expected: 7 + Liqee open orders accounts (MAX_PAIRS) + Liqor open orders accounts (MAX_PAIRS)
    /// 0. `[]` mango_group_ai - MangoGroup
    /// 1. `[]` mango_cache_ai - MangoCache
    /// 2. `[writable]` perp_market_ai - PerpMarket
    /// 3. `[writable]` event_queue_ai - EventQueue
    /// 4. `[writable]` liqee_mango_account_ai - MangoAccount
    /// 5. `[writable]` liqor_mango_account_ai - MangoAccount
    /// 6. `[signer]` liqor_ai - Liqor Account
    /// 7+... `[]` liqee_open_orders_ais - Liqee open orders accs
    /// 7+MAX_PAIRS... `[]` liqor_open_orders_ais - Liqor open orders accs
    LiquidatePerpMarket {
        base_transfer_request: i64,
    },

    /// Take an account that has losses in the selected perp market to account for fees_accrued
    ///
    /// Accounts expected: 10
    /// 0. `[]` mango_group_ai - MangoGroup
    /// 1. `[]` mango_cache_ai - MangoCache
    /// 2. `[writable]` perp_market_ai - PerpMarket
    /// 3. `[writable]` mango_account_ai - MangoAccount
    /// 4. `[]` root_bank_ai - RootBank
    /// 5. `[writable]` node_bank_ai - NodeBank
    /// 6. `[writable]` bank_vault_ai - ?
    /// 7. `[writable]` fees_vault_ai - fee vault owned by mango DAO token governance
    /// 8. `[]` signer_ai - Group Signer Account
    /// 9. `[]` token_prog_ai - Token Program Account
    SettleFees,

    /// Claim insurance fund and then socialize loss
    ///
    /// Accounts expected: 12 + Liqor open orders accounts (MAX_PAIRS)
    /// 0. `[]` mango_group_ai - MangoGroup
    /// 1. `[writable]` mango_cache_ai - MangoCache
    /// 2. `[writable]` liqee_mango_account_ai - Liqee MangoAccount
    /// 3. `[writable]` liqor_mango_account_ai - Liqor MangoAccount
    /// 4. `[signer]` liqor_ai - Liqor Account
    /// 5. `[]` root_bank_ai - RootBank
    /// 6. `[writable]` node_bank_ai - NodeBank
    /// 7. `[writable]` vault_ai - ?
    /// 8. `[writable]` dao_vault_ai - DAO Vault
    /// 9. `[]` signer_ai - Group Signer Account
    /// 10. `[]` perp_market_ai - PerpMarket
    /// 11. `[]` token_prog_ai - Token Program Account
    /// 12+... `[]` liqor_open_orders_ais - Liqor open orders accs
    ResolvePerpBankruptcy {
        // 30
        liab_index: usize,
        max_liab_transfer: I80F48,
    },

    /// Claim insurance fund and then socialize loss
    ///
    /// Accounts expected: 13 + Liqor open orders accounts (MAX_PAIRS) + Liab node banks (MAX_NODE_BANKS)
    /// 0. `[]` mango_group_ai - MangoGroup
    /// 1. `[writable]` mango_cache_ai - MangoCache
    /// 2. `[writable]` liqee_mango_account_ai - Liqee MangoAccount
    /// 3. `[writable]` liqor_mango_account_ai - Liqor MangoAccount
    /// 4. `[signer]` liqor_ai - Liqor Account
    /// 5. `[]` quote_root_bank_ai - RootBank
    /// 6. `[writable]` quote_node_bank_ai - NodeBank
    /// 7. `[writable]` quote_vault_ai - ?
    /// 8. `[writable]` dao_vault_ai - DAO Vault
    /// 9. `[]` signer_ai - Group Signer Account
    /// 10. `[]` liab_root_bank_ai - RootBank
    /// 11. `[writable]` liab_node_bank_ai - NodeBank
    /// 12. `[]` token_prog_ai - Token Program Account
    /// 13+... `[]` liqor_open_orders_ais - Liqor open orders accs
    /// 14+MAX_PAIRS... `[]` liab_node_bank_ais - Lib token node banks
    ResolveTokenBankruptcy {
        max_liab_transfer: I80F48,
    },

    /// Initialize open orders
    ///
    /// Accounts expected by this instruction (8):
    ///
    /// 0. `[]` mango_group_ai - MangoGroup that this mango account is for
    /// 1. `[writable]` mango_account_ai - MangoAccount
    /// 2. `[signer]` owner_ai - MangoAccount owner
    /// 3. `[]` dex_prog_ai - program id of serum dex
    /// 4. `[writable]` open_orders_ai - open orders for this market for this MangoAccount
    /// 5. `[]` spot_market_ai - dex MarketState account
    /// 6. `[]` signer_ai - Group Signer Account
    /// 7. `[]` rent_ai - Rent sysvar account
    InitSpotOpenOrders,

    /// Redeem the mngo_accrued in a PerpAccount for MNGO in MangoAccount deposits
    ///
    /// Accounts expected by this instruction (11):
    /// 0. `[]` mango_group_ai - MangoGroup that this mango account is for
    /// 1. `[]` mango_cache_ai - MangoCache
    /// 2. `[writable]` mango_account_ai - MangoAccount
    /// 3. `[signer]` owner_ai - MangoAccount owner
    /// 4. `[]` perp_market_ai - PerpMarket
    /// 5. `[writable]` mngo_perp_vault_ai
    /// 6. `[]` mngo_root_bank_ai
    /// 7. `[writable]` mngo_node_bank_ai
    /// 8. `[writable]` mngo_bank_vault_ai
    /// 9. `[]` signer_ai - Group Signer Account
    /// 10. `[]` token_prog_ai - SPL Token program id
    RedeemMngo,

    /// Add account info; useful for naming accounts
    ///
    /// Accounts expected by this instruction (3):
    /// 0. `[]` mango_group_ai - MangoGroup that this mango account is for
    /// 1. `[writable]` mango_account_ai - MangoAccount
    /// 2. `[signer]` owner_ai - MangoAccount owner
    AddMangoAccountInfo {
        info: [u8; INFO_LEN],
    },

    /// Deposit MSRM to reduce fees. This MSRM is not at risk and is not used for any health calculations
    ///
    /// Accounts expected by this instruction (6):
    ///
    /// 0. `[]` mango_group_ai - MangoGroup that this mango account is for
    /// 1. `[writable]` mango_account_ai - MangoAccount
    /// 2. `[signer]` owner_ai - MangoAccount owner
    /// 3. `[writable]` msrm_account_ai - MSRM token account
    /// 4. `[writable]` msrm_vault_ai - MSRM vault owned by mango program
    /// 5. `[]` token_prog_ai - SPL Token program id
    DepositMsrm {
        quantity: u64,
    },

    /// Withdraw the MSRM deposited
    ///
    /// Accounts expected by this instruction (7):
    ///
    /// 0. `[]` mango_group_ai - MangoGroup that this mango account is for
    /// 1. `[writable]` mango_account_ai - MangoAccount
    /// 2. `[signer]` owner_ai - MangoAccount owner
    /// 3. `[writable]` msrm_account_ai - MSRM token account
    /// 4. `[writable]` msrm_vault_ai - MSRM vault owned by mango program
    /// 5. `[]` signer_ai - signer key of the MangoGroup
    /// 6. `[]` token_prog_ai - SPL Token program id
    WithdrawMsrm {
        quantity: u64,
    },

    /// Change the params for perp market.
    ///
    /// Accounts expected by this instruction (3):
    /// 0. `[writable]` mango_group_ai - MangoGroup
    /// 1. `[writable]` perp_market_ai - PerpMarket
    /// 2. `[signer]` admin_ai - MangoGroup admin
    ChangePerpMarketParams {
        maint_leverage: Option<I80F48>,
        init_leverage: Option<I80F48>,
        liquidation_fee: Option<I80F48>,
        maker_fee: Option<I80F48>,
        taker_fee: Option<I80F48>,
        /// Starting rate for liquidity mining
        rate: Option<I80F48>,
        /// depth liquidity mining works for
        max_depth_bps: Option<I80F48>,
        /// target length in seconds of one period
        target_period_length: Option<u64>,
        /// amount MNGO rewarded per period
        mngo_per_period: Option<u64>,
    },

    /// Transfer admin permissions over group to another account
    ///
    /// Accounts expected by this instruction (3):
    /// 0. `[writable]` mango_group_ai - MangoGroup
    /// 1. `[]` new_admin_ai - New MangoGroup admin
    /// 2. `[signer]` admin_ai - MangoGroup admin
    SetGroupAdmin,

    /// Cancel all perp open orders (batch cancel)
    ///
    /// Accounts expected: 6
    /// 0. `[]` mango_group_ai - MangoGroup
    /// 1. `[writable]` mango_account_ai - MangoAccount
    /// 2. `[signer]` owner_ai - Owner of Mango Account
    /// 3. `[writable]` perp_market_ai - PerpMarket
    /// 4. `[writable]` bids_ai - Bids acc
    /// 5. `[writable]` asks_ai - Asks acc
    CancelAllPerpOrders {
        limit: u8,
    },

    /// DEPRECATED - will be gone in next release
    /// Liqor takes on all the quote positions where base_position == 0
    /// Equivalent amount of quote currency is credited/debited in deposits/borrows.
    /// This is very similar to the settle_pnl function, but is forced for Sick accounts
    ///
    /// Accounts expected: 7 + MAX_PAIRS
    /// 0. `[]` mango_group_ai - MangoGroup
    /// 1. `[]` mango_cache_ai - MangoCache
    /// 2. `[writable]` liqee_mango_account_ai - MangoAccount
    /// 3. `[writable]` liqor_mango_account_ai - MangoAccount
    /// 4. `[signer]` liqor_ai - Liqor Account
    /// 5. `[]` root_bank_ai - RootBank
    /// 6. `[writable]` node_bank_ai - NodeBank
    /// 7+... `[]` liqee_open_orders_ais - Liqee open orders accs
    ForceSettleQuotePositions,
}

impl MangoInstruction {
    pub fn unpack(input: &[u8]) -> Option<Self> {
        let (&discrim, data) = array_refs![input, 4; ..;];
        let discrim = u32::from_le_bytes(discrim);
        Some(match discrim {
            0 => {
                let data = array_ref![data, 0, 64];
                let (
                    signer_nonce,
                    valid_interval,
                    quote_optimal_util,
                    quote_optimal_rate,
                    quote_max_rate,
                ) = array_refs![data, 8, 8, 16, 16, 16];

                MangoInstruction::InitMangoGroup {
                    signer_nonce: u64::from_le_bytes(*signer_nonce),
                    valid_interval: u64::from_le_bytes(*valid_interval),
                    quote_optimal_util: I80F48::from_le_bytes(*quote_optimal_util),
                    quote_optimal_rate: I80F48::from_le_bytes(*quote_optimal_rate),
                    quote_max_rate: I80F48::from_le_bytes(*quote_max_rate),
                }
            }
            1 => MangoInstruction::InitMangoAccount,
            2 => {
                let quantity = array_ref![data, 0, 8];
                MangoInstruction::Deposit { quantity: u64::from_le_bytes(*quantity) }
            }
            3 => {
                let data = array_ref![data, 0, 9];
                let (quantity, allow_borrow) = array_refs![data, 8, 1];

                let allow_borrow = match allow_borrow {
                    [0] => false,
                    [1] => true,
                    _ => return None,
                };
                MangoInstruction::Withdraw { quantity: u64::from_le_bytes(*quantity), allow_borrow }
            }
            4 => {
                let data = array_ref![data, 0, 96];
                let (
                    maint_leverage,
                    init_leverage,
                    liquidation_fee,
                    optimal_util,
                    optimal_rate,
                    max_rate,
                ) = array_refs![data, 16, 16, 16, 16, 16, 16];
                MangoInstruction::AddSpotMarket {
                    maint_leverage: I80F48::from_le_bytes(*maint_leverage),
                    init_leverage: I80F48::from_le_bytes(*init_leverage),
                    liquidation_fee: I80F48::from_le_bytes(*liquidation_fee),
                    optimal_util: I80F48::from_le_bytes(*optimal_util),
                    optimal_rate: I80F48::from_le_bytes(*optimal_rate),
                    max_rate: I80F48::from_le_bytes(*max_rate),
                }
            }
            5 => {
                let market_index = array_ref![data, 0, 8];
                MangoInstruction::AddToBasket { market_index: usize::from_le_bytes(*market_index) }
            }
            6 => {
                let quantity = array_ref![data, 0, 8];
                MangoInstruction::Borrow { quantity: u64::from_le_bytes(*quantity) }
            }
            7 => MangoInstruction::CachePrices,
            8 => MangoInstruction::CacheRootBanks,
            9 => {
                let data_arr = array_ref![data, 0, 46];
                let order = unpack_dex_new_order_v3(data_arr)?;
                MangoInstruction::PlaceSpotOrder { order }
            }
            10 => MangoInstruction::AddOracle,
            11 => {
                let data_arr = array_ref![data, 0, 144];
                let (
                    maint_leverage,
                    init_leverage,
                    liquidation_fee,
                    maker_fee,
                    taker_fee,
                    base_lot_size,
                    quote_lot_size,
                    rate,
                    max_depth_bps,
                    target_period_length,
                    mngo_per_period,
                ) = array_refs![data_arr, 16, 16, 16, 16, 16, 8, 8, 16, 16, 8, 8];
                MangoInstruction::AddPerpMarket {
                    maint_leverage: I80F48::from_le_bytes(*maint_leverage),
                    init_leverage: I80F48::from_le_bytes(*init_leverage),
                    liquidation_fee: I80F48::from_le_bytes(*liquidation_fee),
                    maker_fee: I80F48::from_le_bytes(*maker_fee),
                    taker_fee: I80F48::from_le_bytes(*taker_fee),
                    base_lot_size: i64::from_le_bytes(*base_lot_size),
                    quote_lot_size: i64::from_le_bytes(*quote_lot_size),
                    rate: I80F48::from_le_bytes(*rate),
                    max_depth_bps: I80F48::from_le_bytes(*max_depth_bps),
                    target_period_length: u64::from_le_bytes(*target_period_length),
                    mngo_per_period: u64::from_le_bytes(*mngo_per_period),
                }
            }
            12 => {
                let data_arr = array_ref![data, 0, 26];
                let (price, quantity, client_order_id, side, order_type) =
                    array_refs![data_arr, 8, 8, 8, 1, 1];
                MangoInstruction::PlacePerpOrder {
                    price: i64::from_le_bytes(*price),
                    quantity: i64::from_le_bytes(*quantity),
                    client_order_id: u64::from_le_bytes(*client_order_id),
                    side: Side::try_from_primitive(side[0]).ok()?,
                    order_type: OrderType::try_from_primitive(order_type[0]).ok()?,
                }
            }
            13 => {
                // ***
                let data_arr = array_ref![data, 0, 9];
                let (client_order_id, invalid_id_ok) = array_refs![data_arr, 8, 1];

                MangoInstruction::CancelPerpOrderByClientId {
                    client_order_id: u64::from_le_bytes(*client_order_id),
                    invalid_id_ok: invalid_id_ok[0] != 0,
                }
            }
            14 => {
                // ***
                let data_arr = array_ref![data, 0, 17];
                let (order_id, invalid_id_ok) = array_refs![data_arr, 16, 1];
                MangoInstruction::CancelPerpOrder {
                    order_id: i128::from_le_bytes(*order_id),
                    invalid_id_ok: invalid_id_ok[0] != 0,
                }
            }
            15 => {
                let data_arr = array_ref![data, 0, 8];
                MangoInstruction::ConsumeEvents { limit: usize::from_le_bytes(*data_arr) }
            }
            16 => MangoInstruction::CachePerpMarkets,
            17 => MangoInstruction::UpdateFunding,
            18 => {
                let data_arr = array_ref![data, 0, 16];
                MangoInstruction::SetOracle { price: I80F48::from_le_bytes(*data_arr) }
            }
            19 => MangoInstruction::SettleFunds,
            20 => {
                let data_array = array_ref![data, 0, 20];
                let fields = array_refs![data_array, 4, 16];
                let side = match u32::from_le_bytes(*fields.0) {
                    0 => serum_dex::matching::Side::Bid,
                    1 => serum_dex::matching::Side::Ask,
                    _ => return None,
                };
                let order_id = u128::from_le_bytes(*fields.1);
                let order = serum_dex::instruction::CancelOrderInstructionV2 { side, order_id };
                MangoInstruction::CancelSpotOrder { order }
            }
            21 => MangoInstruction::UpdateRootBank,

            22 => {
                let data_arr = array_ref![data, 0, 8];

                MangoInstruction::SettlePnl { market_index: usize::from_le_bytes(*data_arr) }
            }
            23 => {
                let data = array_ref![data, 0, 16];
                let (token_index, quantity) = array_refs![data, 8, 8];

                MangoInstruction::SettleBorrow {
                    token_index: usize::from_le_bytes(*token_index),
                    quantity: u64::from_le_bytes(*quantity),
                }
            }
            24 => {
                let data_arr = array_ref![data, 0, 1];

                MangoInstruction::ForceCancelSpotOrders { limit: u8::from_le_bytes(*data_arr) }
            }
            25 => {
                let data_arr = array_ref![data, 0, 1];

                MangoInstruction::ForceCancelPerpOrders { limit: u8::from_le_bytes(*data_arr) }
            }
            26 => {
                let data_arr = array_ref![data, 0, 16];

                MangoInstruction::LiquidateTokenAndToken {
                    max_liab_transfer: I80F48::from_le_bytes(*data_arr),
                }
            }
            27 => {
                let data = array_ref![data, 0, 34];
                let (asset_type, asset_index, liab_type, liab_index, max_liab_transfer) =
                    array_refs![data, 1, 8, 1, 8, 16];

                MangoInstruction::LiquidateTokenAndPerp {
                    asset_type: AssetType::try_from(u8::from_le_bytes(*asset_type)).unwrap(),
                    asset_index: usize::from_le_bytes(*asset_index),
                    liab_type: AssetType::try_from(u8::from_le_bytes(*liab_type)).unwrap(),
                    liab_index: usize::from_le_bytes(*liab_index),
                    max_liab_transfer: I80F48::from_le_bytes(*max_liab_transfer),
                }
            }
            28 => {
                let data_arr = array_ref![data, 0, 8];

                MangoInstruction::LiquidatePerpMarket {
                    base_transfer_request: i64::from_le_bytes(*data_arr),
                }
            }
            29 => MangoInstruction::SettleFees,
            30 => {
                let data = array_ref![data, 0, 24];
                let (liab_index, max_liab_transfer) = array_refs![data, 8, 16];

                MangoInstruction::ResolvePerpBankruptcy {
                    liab_index: usize::from_le_bytes(*liab_index),
                    max_liab_transfer: I80F48::from_le_bytes(*max_liab_transfer),
                }
            }
            31 => {
                let data_arr = array_ref![data, 0, 16];

                MangoInstruction::ResolveTokenBankruptcy {
                    max_liab_transfer: I80F48::from_le_bytes(*data_arr),
                }
            }
            32 => MangoInstruction::InitSpotOpenOrders,
            33 => MangoInstruction::RedeemMngo,
            34 => {
                let info = array_ref![data, 0, INFO_LEN];
                MangoInstruction::AddMangoAccountInfo { info: *info }
            }
            35 => {
                let quantity = array_ref![data, 0, 8];
                MangoInstruction::DepositMsrm { quantity: u64::from_le_bytes(*quantity) }
            }
            36 => {
                let quantity = array_ref![data, 0, 8];
                MangoInstruction::WithdrawMsrm { quantity: u64::from_le_bytes(*quantity) }
            }

            37 => {
                let data_arr = array_ref![data, 0, 137];
                let (
                    maint_leverage,
                    init_leverage,
                    liquidation_fee,
                    maker_fee,
                    taker_fee,
                    rate,
                    max_depth_bps,
                    target_period_length,
                    mngo_per_period,
                ) = array_refs![data_arr, 17, 17, 17, 17, 17, 17, 17, 9, 9];

                MangoInstruction::ChangePerpMarketParams {
                    maint_leverage: unpack_i80f48_opt(maint_leverage),
                    init_leverage: unpack_i80f48_opt(init_leverage),
                    liquidation_fee: unpack_i80f48_opt(liquidation_fee),
                    maker_fee: unpack_i80f48_opt(maker_fee),
                    taker_fee: unpack_i80f48_opt(taker_fee),
                    rate: unpack_i80f48_opt(rate),
                    max_depth_bps: unpack_i80f48_opt(max_depth_bps),
                    target_period_length: unpack_u64_opt(target_period_length),
                    mngo_per_period: unpack_u64_opt(mngo_per_period),
                }
            }

            38 => MangoInstruction::SetGroupAdmin,

            39 => {
                let data_arr = array_ref![data, 0, 1];
                MangoInstruction::CancelAllPerpOrders { limit: u8::from_le_bytes(*data_arr) }
            }

            40 => MangoInstruction::ForceSettleQuotePositions,

            _ => {
                return None;
            }
        })
    }
    pub fn pack(&self) -> Vec<u8> {
        bincode::serialize(self).unwrap()
    }
}

fn unpack_i80f48_opt(data: &[u8; 17]) -> Option<I80F48> {
    let (opt, val) = array_refs![data, 1, 16];
    if opt[0] == 0 {
        None
    } else {
        Some(I80F48::from_le_bytes(*val))
    }
}
fn unpack_u64_opt(data: &[u8; 9]) -> Option<u64> {
    let (opt, val) = array_refs![data, 1, 8];
    if opt[0] == 0 {
        None
    } else {
        Some(u64::from_le_bytes(*val))
    }
}

fn unpack_dex_new_order_v3(
    data: &[u8; 46],
) -> Option<serum_dex::instruction::NewOrderInstructionV3> {
    let (
        &side_arr,
        &price_arr,
        &max_coin_qty_arr,
        &max_native_pc_qty_arr,
        &self_trade_behavior_arr,
        &otype_arr,
        &client_order_id_bytes,
        &limit_arr,
    ) = array_refs![data, 4, 8, 8, 8, 4, 4, 8, 2];

    let side = serum_dex::matching::Side::try_from_primitive(
        u32::from_le_bytes(side_arr).try_into().ok()?,
    )
    .ok()?;
    let limit_price = NonZeroU64::new(u64::from_le_bytes(price_arr))?;
    let max_coin_qty = NonZeroU64::new(u64::from_le_bytes(max_coin_qty_arr))?;
    let max_native_pc_qty_including_fees =
        NonZeroU64::new(u64::from_le_bytes(max_native_pc_qty_arr))?;
    let self_trade_behavior = serum_dex::instruction::SelfTradeBehavior::try_from_primitive(
        u32::from_le_bytes(self_trade_behavior_arr).try_into().ok()?,
    )
    .ok()?;
    let order_type = serum_dex::matching::OrderType::try_from_primitive(
        u32::from_le_bytes(otype_arr).try_into().ok()?,
    )
    .ok()?;
    let client_order_id = u64::from_le_bytes(client_order_id_bytes);
    let limit = u16::from_le_bytes(limit_arr);

    Some(serum_dex::instruction::NewOrderInstructionV3 {
        side,
        limit_price,
        max_coin_qty,
        max_native_pc_qty_including_fees,
        self_trade_behavior,
        order_type,
        client_order_id,
        limit,
    })
}

pub fn init_mango_group(
    program_id: &Pubkey,
    mango_group_pk: &Pubkey,
    signer_pk: &Pubkey,
    admin_pk: &Pubkey,
    quote_mint_pk: &Pubkey,
    quote_vault_pk: &Pubkey,
    quote_node_bank_pk: &Pubkey,
    quote_root_bank_pk: &Pubkey,
    insurance_vault_pk: &Pubkey,
    msrm_vault_pk: &Pubkey, // send in Pubkey:default() if not using this feature
    fees_vault_pk: &Pubkey,
    mango_cache_ai: &Pubkey,
    dex_program_pk: &Pubkey,

    signer_nonce: u64,
    valid_interval: u64,
    quote_optimal_util: I80F48,
    quote_optimal_rate: I80F48,
    quote_max_rate: I80F48,
) -> Result<Instruction, ProgramError> {
    let accounts = vec![
        AccountMeta::new(*mango_group_pk, false),
        AccountMeta::new_readonly(*signer_pk, false),
        AccountMeta::new_readonly(*admin_pk, true),
        AccountMeta::new_readonly(*quote_mint_pk, false),
        AccountMeta::new_readonly(*quote_vault_pk, false),
        AccountMeta::new(*quote_node_bank_pk, false),
        AccountMeta::new(*quote_root_bank_pk, false),
        AccountMeta::new_readonly(*insurance_vault_pk, false),
        AccountMeta::new_readonly(*msrm_vault_pk, false),
        AccountMeta::new_readonly(*fees_vault_pk, false),
        AccountMeta::new(*mango_cache_ai, false),
        AccountMeta::new_readonly(*dex_program_pk, false),
    ];

    let instr = MangoInstruction::InitMangoGroup {
        signer_nonce,
        valid_interval,
        quote_optimal_util,
        quote_optimal_rate,
        quote_max_rate,
    };

    let data = instr.pack();
    Ok(Instruction { program_id: *program_id, accounts, data })
}

pub fn init_mango_account(
    program_id: &Pubkey,
    mango_group_pk: &Pubkey,
    mango_account_pk: &Pubkey,
    owner_pk: &Pubkey,
) -> Result<Instruction, ProgramError> {
    let accounts = vec![
        AccountMeta::new_readonly(*mango_group_pk, false),
        AccountMeta::new(*mango_account_pk, false),
        AccountMeta::new_readonly(*owner_pk, true),
        AccountMeta::new_readonly(solana_program::sysvar::rent::ID, false),
    ];

    let instr = MangoInstruction::InitMangoAccount;
    let data = instr.pack();
    Ok(Instruction { program_id: *program_id, accounts, data })
}

pub fn deposit(
    program_id: &Pubkey,
    mango_group_pk: &Pubkey,
    mango_account_pk: &Pubkey,
    owner_pk: &Pubkey,
    mango_cache_pk: &Pubkey,
    root_bank_pk: &Pubkey,
    node_bank_pk: &Pubkey,
    vault_pk: &Pubkey,
    owner_token_account_pk: &Pubkey,

    quantity: u64,
) -> Result<Instruction, ProgramError> {
    let accounts = vec![
        AccountMeta::new_readonly(*mango_group_pk, false),
        AccountMeta::new(*mango_account_pk, false),
        AccountMeta::new_readonly(*owner_pk, true),
        AccountMeta::new_readonly(*mango_cache_pk, false),
        AccountMeta::new_readonly(*root_bank_pk, false),
        AccountMeta::new(*node_bank_pk, false),
        AccountMeta::new(*vault_pk, false),
        AccountMeta::new_readonly(spl_token::ID, false),
        AccountMeta::new(*owner_token_account_pk, false),
    ];

    let instr = MangoInstruction::Deposit { quantity };
    let data = instr.pack();
    Ok(Instruction { program_id: *program_id, accounts, data })
}

pub fn add_spot_market(
    program_id: &Pubkey,
    mango_group_pk: &Pubkey,
    oracle_pk: &Pubkey,
    spot_market_pk: &Pubkey,
    dex_program_pk: &Pubkey,
    token_mint_pk: &Pubkey,
    node_bank_pk: &Pubkey,
    vault_pk: &Pubkey,
    root_bank_pk: &Pubkey,
    admin_pk: &Pubkey,

    maint_leverage: I80F48,
    init_leverage: I80F48,
    liquidation_fee: I80F48,
    optimal_util: I80F48,
    optimal_rate: I80F48,
    max_rate: I80F48,
) -> Result<Instruction, ProgramError> {
    let accounts = vec![
        AccountMeta::new(*mango_group_pk, false),
        AccountMeta::new_readonly(*oracle_pk, false),
        AccountMeta::new_readonly(*spot_market_pk, false),
        AccountMeta::new_readonly(*dex_program_pk, false),
        AccountMeta::new_readonly(*token_mint_pk, false),
        AccountMeta::new(*node_bank_pk, false),
        AccountMeta::new_readonly(*vault_pk, false),
        AccountMeta::new(*root_bank_pk, false),
        AccountMeta::new_readonly(*admin_pk, true),
    ];

    let instr = MangoInstruction::AddSpotMarket {
        maint_leverage,
        init_leverage,
        liquidation_fee,
        optimal_util,
        optimal_rate,
        max_rate,
    };
    let data = instr.pack();
    Ok(Instruction { program_id: *program_id, accounts, data })
}

pub fn add_perp_market(
    program_id: &Pubkey,
    mango_group_pk: &Pubkey,
    oracle_pk: &Pubkey,
    perp_market_pk: &Pubkey,
    event_queue_pk: &Pubkey,
    bids_pk: &Pubkey,
    asks_pk: &Pubkey,
    mngo_vault_pk: &Pubkey,
    admin_pk: &Pubkey,

    maint_leverage: I80F48,
    init_leverage: I80F48,
    liquidation_fee: I80F48,
    maker_fee: I80F48,
    taker_fee: I80F48,
    base_lot_size: i64,
    quote_lot_size: i64,
    rate: I80F48,
    max_depth_bps: I80F48,
    target_period_length: u64,
    mngo_per_period: u64,
) -> Result<Instruction, ProgramError> {
    let accounts = vec![
        AccountMeta::new(*mango_group_pk, false),
        AccountMeta::new(*oracle_pk, false),
        AccountMeta::new(*perp_market_pk, false),
        AccountMeta::new(*event_queue_pk, false),
        AccountMeta::new(*bids_pk, false),
        AccountMeta::new(*asks_pk, false),
        AccountMeta::new_readonly(*mngo_vault_pk, false),
        AccountMeta::new_readonly(*admin_pk, true),
    ];

    let instr = MangoInstruction::AddPerpMarket {
        maint_leverage,
        init_leverage,
        liquidation_fee,
        maker_fee,
        taker_fee,
        base_lot_size,
        quote_lot_size,
        rate,
        max_depth_bps,
        target_period_length,
        mngo_per_period,
    };
    let data = instr.pack();
    Ok(Instruction { program_id: *program_id, accounts, data })
}

pub fn place_perp_order(
    program_id: &Pubkey,
    mango_group_pk: &Pubkey,
    mango_account_pk: &Pubkey,
    owner_pk: &Pubkey,
    mango_cache_pk: &Pubkey,
    perp_market_pk: &Pubkey,
    bids_pk: &Pubkey,
    asks_pk: &Pubkey,
    event_queue_pk: &Pubkey,
    open_orders_pks: &[Pubkey; MAX_PAIRS],
    side: Side,
    price: i64,
    quantity: i64,
    client_order_id: u64,
    order_type: OrderType,
) -> Result<Instruction, ProgramError> {
    let mut accounts = vec![
        AccountMeta::new_readonly(*mango_group_pk, false),
        AccountMeta::new(*mango_account_pk, false),
        AccountMeta::new_readonly(*owner_pk, true),
        AccountMeta::new_readonly(*mango_cache_pk, false),
        AccountMeta::new(*perp_market_pk, false),
        AccountMeta::new(*bids_pk, false),
        AccountMeta::new(*asks_pk, false),
        AccountMeta::new(*event_queue_pk, false),
    ];
    accounts.extend(open_orders_pks.iter().map(|pk| AccountMeta::new_readonly(*pk, false)));

    let instr =
        MangoInstruction::PlacePerpOrder { side, price, quantity, client_order_id, order_type };
    let data = instr.pack();

    Ok(Instruction { program_id: *program_id, accounts, data })
}

pub fn cancel_perp_order_by_client_id(
    program_id: &Pubkey,
    mango_group_pk: &Pubkey,   // read
    mango_account_pk: &Pubkey, // write
    owner_pk: &Pubkey,         // read, signer
    perp_market_pk: &Pubkey,   // write
    bids_pk: &Pubkey,          // write
    asks_pk: &Pubkey,          // write
    client_order_id: u64,
    invalid_id_ok: bool,
) -> Result<Instruction, ProgramError> {
    let accounts = vec![
        AccountMeta::new_readonly(*mango_group_pk, false),
        AccountMeta::new(*mango_account_pk, false),
        AccountMeta::new_readonly(*owner_pk, true),
        AccountMeta::new(*perp_market_pk, false),
        AccountMeta::new(*bids_pk, false),
        AccountMeta::new(*asks_pk, false),
    ];
    let instr = MangoInstruction::CancelPerpOrderByClientId { client_order_id, invalid_id_ok };
    let data = instr.pack();
    Ok(Instruction { program_id: *program_id, accounts, data })
}

pub fn cancel_perp_order(
    program_id: &Pubkey,
    mango_group_pk: &Pubkey,   // read
    mango_account_pk: &Pubkey, // write
    owner_pk: &Pubkey,         // read, signer
    perp_market_pk: &Pubkey,   // write
    bids_pk: &Pubkey,          // write
    asks_pk: &Pubkey,          // write
    order_id: i128,
    invalid_id_ok: bool,
) -> Result<Instruction, ProgramError> {
    let accounts = vec![
        AccountMeta::new_readonly(*mango_group_pk, false),
        AccountMeta::new(*mango_account_pk, false),
        AccountMeta::new_readonly(*owner_pk, true),
        AccountMeta::new(*perp_market_pk, false),
        AccountMeta::new(*bids_pk, false),
        AccountMeta::new(*asks_pk, false),
    ];
    let instr = MangoInstruction::CancelPerpOrder { order_id, invalid_id_ok };
    let data = instr.pack();
    Ok(Instruction { program_id: *program_id, accounts, data })
}

pub fn cancel_all_perp_orders(
    program_id: &Pubkey,
    mango_group_pk: &Pubkey,   // read
    mango_account_pk: &Pubkey, // write
    owner_pk: &Pubkey,         // read, signer
    perp_market_pk: &Pubkey,   // write
    bids_pk: &Pubkey,          // write
    asks_pk: &Pubkey,          // write
    limit: u8,
) -> Result<Instruction, ProgramError> {
    let accounts = vec![
        AccountMeta::new_readonly(*mango_group_pk, false),
        AccountMeta::new(*mango_account_pk, false),
        AccountMeta::new_readonly(*owner_pk, true),
        AccountMeta::new(*perp_market_pk, false),
        AccountMeta::new(*bids_pk, false),
        AccountMeta::new(*asks_pk, false),
    ];
    let instr = MangoInstruction::CancelAllPerpOrders { limit };
    let data = instr.pack();
    Ok(Instruction { program_id: *program_id, accounts, data })
}

pub fn force_cancel_perp_orders(
    program_id: &Pubkey,
    mango_group_pk: &Pubkey,         // read
    mango_cache_pk: &Pubkey,         // read
    perp_market_pk: &Pubkey,         // read
    bids_pk: &Pubkey,                // write
    asks_pk: &Pubkey,                // write
    liqee_mango_account_pk: &Pubkey, // write
    open_orders_pks: &[Pubkey],      // read
    limit: u8,
) -> Result<Instruction, ProgramError> {
    let mut accounts = vec![
        AccountMeta::new_readonly(*mango_group_pk, false),
        AccountMeta::new_readonly(*mango_cache_pk, false),
        AccountMeta::new_readonly(*perp_market_pk, false),
        AccountMeta::new(*bids_pk, false),
        AccountMeta::new(*asks_pk, false),
        AccountMeta::new(*liqee_mango_account_pk, false),
    ];
    accounts.extend(open_orders_pks.iter().map(|pk| AccountMeta::new_readonly(*pk, false)));
    let instr = MangoInstruction::ForceCancelPerpOrders { limit };
    let data = instr.pack();
    Ok(Instruction { program_id: *program_id, accounts, data })
}

pub fn consume_events(
    program_id: &Pubkey,
    mango_group_pk: &Pubkey,      // read
    mango_cache_pk: &Pubkey,      // read
    perp_market_pk: &Pubkey,      // read
    event_queue_pk: &Pubkey,      // write
    mango_acc_pks: &mut [Pubkey], // write
    limit: usize,
) -> Result<Instruction, ProgramError> {
    let fixed_accounts = vec![
        AccountMeta::new_readonly(*mango_group_pk, false),
        AccountMeta::new_readonly(*mango_cache_pk, false),
        AccountMeta::new(*perp_market_pk, false),
        AccountMeta::new(*event_queue_pk, false),
    ];
    mango_acc_pks.sort();
    let mango_accounts = mango_acc_pks.into_iter().map(|pk| AccountMeta::new(*pk, false));
    let accounts = fixed_accounts.into_iter().chain(mango_accounts).collect();
    let instr = MangoInstruction::ConsumeEvents { limit };
    let data = instr.pack();
    Ok(Instruction { program_id: *program_id, accounts, data })
}

pub fn settle_pnl(
    program_id: &Pubkey,
    mango_group_pk: &Pubkey,     // read
    mango_account_a_pk: &Pubkey, // write
    mango_account_b_pk: &Pubkey, // write
    mango_cache_pk: &Pubkey,     // read
    root_bank_pk: &Pubkey,       // read
    node_bank_pk: &Pubkey,       // write
    market_index: usize,
) -> Result<Instruction, ProgramError> {
    let accounts = vec![
        AccountMeta::new_readonly(*mango_group_pk, false),
        AccountMeta::new(*mango_account_a_pk, false),
        AccountMeta::new(*mango_account_b_pk, false),
        AccountMeta::new_readonly(*mango_cache_pk, false),
        AccountMeta::new_readonly(*root_bank_pk, false),
        AccountMeta::new(*node_bank_pk, false),
    ];
    let instr = MangoInstruction::SettlePnl { market_index };
    let data = instr.pack();
    Ok(Instruction { program_id: *program_id, accounts, data })
}

pub fn update_funding(
    program_id: &Pubkey,
    mango_group_pk: &Pubkey, // read
    mango_cache_pk: &Pubkey, // read
    perp_market_pk: &Pubkey, // write
    bids_pk: &Pubkey,        // read
    asks_pk: &Pubkey,        // read
) -> Result<Instruction, ProgramError> {
    let accounts = vec![
        AccountMeta::new_readonly(*mango_group_pk, false),
        AccountMeta::new_readonly(*mango_cache_pk, false),
        AccountMeta::new(*perp_market_pk, false),
        AccountMeta::new_readonly(*bids_pk, false),
        AccountMeta::new_readonly(*asks_pk, false),
    ];
    let instr = MangoInstruction::UpdateFunding {};
    let data = instr.pack();
    Ok(Instruction { program_id: *program_id, accounts, data })
}

pub fn withdraw(
    program_id: &Pubkey,
    mango_group_pk: &Pubkey,
    mango_account_pk: &Pubkey,
    owner_pk: &Pubkey,
    mango_cache_pk: &Pubkey,
    root_bank_pk: &Pubkey,
    node_bank_pk: &Pubkey,
    vault_pk: &Pubkey,
    token_account_pk: &Pubkey,
    signer_pk: &Pubkey,
    open_orders_pks: &[Pubkey],

    quantity: u64,
    allow_borrow: bool,
) -> Result<Instruction, ProgramError> {
    let mut accounts = vec![
        AccountMeta::new(*mango_group_pk, false),
        AccountMeta::new(*mango_account_pk, false),
        AccountMeta::new_readonly(*owner_pk, true),
        AccountMeta::new_readonly(*mango_cache_pk, false),
        AccountMeta::new_readonly(*root_bank_pk, false),
        AccountMeta::new(*node_bank_pk, false),
        AccountMeta::new(*vault_pk, false),
        AccountMeta::new(*token_account_pk, false),
        AccountMeta::new_readonly(*signer_pk, false),
        AccountMeta::new_readonly(spl_token::ID, false),
    ];

    accounts.extend(open_orders_pks.iter().map(|pk| AccountMeta::new_readonly(*pk, false)));

    let instr = MangoInstruction::Withdraw { quantity, allow_borrow };
    let data = instr.pack();
    Ok(Instruction { program_id: *program_id, accounts, data })
}

pub fn borrow(
    program_id: &Pubkey,
    mango_group_pk: &Pubkey,
    mango_account_pk: &Pubkey,
    mango_cache_pk: &Pubkey,
    owner_pk: &Pubkey,
    root_bank_pk: &Pubkey,
    node_bank_pk: &Pubkey,
    open_orders_pks: &[Pubkey],

    quantity: u64,
) -> Result<Instruction, ProgramError> {
    let mut accounts = vec![
        AccountMeta::new(*mango_group_pk, false),
        AccountMeta::new(*mango_account_pk, false),
        AccountMeta::new_readonly(*owner_pk, true),
        AccountMeta::new_readonly(*mango_cache_pk, false),
        AccountMeta::new_readonly(*root_bank_pk, false),
        AccountMeta::new(*node_bank_pk, false),
    ];

    accounts.extend(open_orders_pks.iter().map(|pk| AccountMeta::new(*pk, false)));

    let instr = MangoInstruction::Borrow { quantity };
    let data = instr.pack();
    Ok(Instruction { program_id: *program_id, accounts, data })
}

pub fn cache_prices(
    program_id: &Pubkey,
    mango_group_pk: &Pubkey,
    mango_cache_pk: &Pubkey,
    oracle_pks: &[Pubkey],
) -> Result<Instruction, ProgramError> {
    let mut accounts = vec![
        AccountMeta::new_readonly(*mango_group_pk, false),
        AccountMeta::new(*mango_cache_pk, false),
    ];
    accounts.extend(oracle_pks.iter().map(|pk| AccountMeta::new_readonly(*pk, false)));
    let instr = MangoInstruction::CachePrices;
    let data = instr.pack();
    Ok(Instruction { program_id: *program_id, accounts, data })
}

pub fn cache_root_banks(
    program_id: &Pubkey,
    mango_group_pk: &Pubkey,
    mango_cache_pk: &Pubkey,
    root_bank_pks: &[Pubkey],
) -> Result<Instruction, ProgramError> {
    let mut accounts = vec![
        AccountMeta::new_readonly(*mango_group_pk, false),
        AccountMeta::new(*mango_cache_pk, false),
    ];
    accounts.extend(root_bank_pks.iter().map(|pk| AccountMeta::new_readonly(*pk, false)));
    let instr = MangoInstruction::CacheRootBanks;
    let data = instr.pack();
    Ok(Instruction { program_id: *program_id, accounts, data })
}

pub fn cache_perp_markets(
    program_id: &Pubkey,
    mango_group_pk: &Pubkey,
    mango_cache_pk: &Pubkey,
    perp_market_pks: &[Pubkey],
) -> Result<Instruction, ProgramError> {
    let mut accounts = vec![
        AccountMeta::new_readonly(*mango_group_pk, false),
        AccountMeta::new(*mango_cache_pk, false),
    ];
    accounts.extend(perp_market_pks.iter().map(|pk| AccountMeta::new_readonly(*pk, false)));
    let instr = MangoInstruction::CachePerpMarkets;
    let data = instr.pack();
    Ok(Instruction { program_id: *program_id, accounts, data })
}

pub fn init_spot_open_orders(
    program_id: &Pubkey,
    mango_group_pk: &Pubkey,
    mango_account_pk: &Pubkey,
    owner_pk: &Pubkey,
    dex_prog_pk: &Pubkey,
    open_orders_pk: &Pubkey,
    spot_market_pk: &Pubkey,
    signer_pk: &Pubkey,
) -> Result<Instruction, ProgramError> {
    let accounts = vec![
        AccountMeta::new_readonly(*mango_group_pk, false),
        AccountMeta::new(*mango_account_pk, false),
        AccountMeta::new_readonly(*owner_pk, true),
        AccountMeta::new_readonly(*dex_prog_pk, false),
        AccountMeta::new(*open_orders_pk, false),
        AccountMeta::new_readonly(*spot_market_pk, false),
        AccountMeta::new_readonly(*signer_pk, false),
        AccountMeta::new_readonly(solana_program::sysvar::rent::ID, false),
    ];

    let instr = MangoInstruction::InitSpotOpenOrders;
    let data = instr.pack();

    Ok(Instruction { program_id: *program_id, accounts, data })
}

pub fn place_spot_order(
    program_id: &Pubkey,
    mango_group_pk: &Pubkey,
    mango_account_pk: &Pubkey,
    owner_pk: &Pubkey,
    mango_cache_pk: &Pubkey,
    dex_prog_pk: &Pubkey,
    spot_market_pk: &Pubkey,
    bids_pk: &Pubkey,
    asks_pk: &Pubkey,
    dex_request_queue_pk: &Pubkey,
    dex_event_queue_pk: &Pubkey,
    dex_base_pk: &Pubkey,
    dex_quote_pk: &Pubkey,
    base_root_bank_pk: &Pubkey,
    base_node_bank_pk: &Pubkey,
    base_vault_pk: &Pubkey,
    quote_root_bank_pk: &Pubkey,
    quote_node_bank_pk: &Pubkey,
    quote_vault_pk: &Pubkey,
    signer_pk: &Pubkey,
    dex_signer_pk: &Pubkey,
    msrm_or_srm_vault_pk: &Pubkey,
    open_orders_pks: &[Pubkey],

    market_index: usize, // used to determine which of the open orders accounts should be passed in write
    order: serum_dex::instruction::NewOrderInstructionV3,
) -> Result<Instruction, ProgramError> {
    let mut accounts = vec![
        AccountMeta::new_readonly(*mango_group_pk, false),
        AccountMeta::new(*mango_account_pk, false),
        AccountMeta::new_readonly(*owner_pk, true),
        AccountMeta::new_readonly(*mango_cache_pk, false),
        AccountMeta::new_readonly(*dex_prog_pk, false),
        AccountMeta::new(*spot_market_pk, false),
        AccountMeta::new(*bids_pk, false),
        AccountMeta::new(*asks_pk, false),
        AccountMeta::new(*dex_request_queue_pk, false),
        AccountMeta::new(*dex_event_queue_pk, false),
        AccountMeta::new(*dex_base_pk, false),
        AccountMeta::new(*dex_quote_pk, false),
        AccountMeta::new_readonly(*base_root_bank_pk, false),
        AccountMeta::new(*base_node_bank_pk, false),
        AccountMeta::new(*base_vault_pk, false),
        AccountMeta::new_readonly(*quote_root_bank_pk, false),
        AccountMeta::new(*quote_node_bank_pk, false),
        AccountMeta::new(*quote_vault_pk, false),
        AccountMeta::new_readonly(spl_token::ID, false),
        AccountMeta::new_readonly(*signer_pk, false),
        AccountMeta::new_readonly(solana_program::sysvar::rent::ID, false),
        AccountMeta::new_readonly(*dex_signer_pk, false),
        AccountMeta::new_readonly(*msrm_or_srm_vault_pk, false),
    ];

    accounts.extend(open_orders_pks.iter().enumerate().map(|(i, pk)| {
        if i == market_index {
            AccountMeta::new(*pk, false)
        } else {
            AccountMeta::new_readonly(*pk, false)
        }
    }));

    let instr = MangoInstruction::PlaceSpotOrder { order };
    let data = instr.pack();

    Ok(Instruction { program_id: *program_id, accounts, data })
}

pub fn settle_funds(
    program_id: &Pubkey,
    mango_group_pk: &Pubkey,
    mango_cache_pk: &Pubkey,
    owner_pk: &Pubkey,
    mango_account_pk: &Pubkey,
    dex_prog_pk: &Pubkey,
    spot_market_pk: &Pubkey,
    open_orders_pk: &Pubkey,
    signer_pk: &Pubkey,
    dex_base_pk: &Pubkey,
    dex_quote_pk: &Pubkey,
    base_root_bank_pk: &Pubkey,
    base_node_bank_pk: &Pubkey,
    quote_root_bank_pk: &Pubkey,
    quote_node_bank_pk: &Pubkey,
    base_vault_pk: &Pubkey,
    quote_vault_pk: &Pubkey,
    dex_signer_pk: &Pubkey,
) -> Result<Instruction, ProgramError> {
    let accounts = vec![
        AccountMeta::new_readonly(*mango_group_pk, false),
        AccountMeta::new_readonly(*mango_cache_pk, false),
        AccountMeta::new_readonly(*owner_pk, true),
        AccountMeta::new(*mango_account_pk, false),
        AccountMeta::new_readonly(*dex_prog_pk, false),
        AccountMeta::new(*spot_market_pk, false),
        AccountMeta::new(*open_orders_pk, false),
        AccountMeta::new_readonly(*signer_pk, false),
        AccountMeta::new(*dex_base_pk, false),
        AccountMeta::new(*dex_quote_pk, false),
        AccountMeta::new_readonly(*base_root_bank_pk, false),
        AccountMeta::new(*base_node_bank_pk, false),
        AccountMeta::new_readonly(*quote_root_bank_pk, false),
        AccountMeta::new(*quote_node_bank_pk, false),
        AccountMeta::new(*base_vault_pk, false),
        AccountMeta::new(*quote_vault_pk, false),
        AccountMeta::new_readonly(*dex_signer_pk, false),
        AccountMeta::new_readonly(spl_token::ID, false),
    ];

    let instr = MangoInstruction::SettleFunds;
    let data = instr.pack();

    Ok(Instruction { program_id: *program_id, accounts, data })
}

pub fn add_oracle(
    program_id: &Pubkey,
    mango_group_pk: &Pubkey,
    oracle_pk: &Pubkey,
    admin_pk: &Pubkey,
) -> Result<Instruction, ProgramError> {
    let accounts = vec![
        AccountMeta::new(*mango_group_pk, false),
        AccountMeta::new(*oracle_pk, false),
        AccountMeta::new_readonly(*admin_pk, true),
    ];

    let instr = MangoInstruction::AddOracle;
    let data = instr.pack();
    Ok(Instruction { program_id: *program_id, accounts, data })
}

pub fn update_root_bank(
    program_id: &Pubkey,
    mango_group_pk: &Pubkey,
    mango_cache_pk: &Pubkey,
    root_bank_pk: &Pubkey,
    node_bank_pks: &[Pubkey],
) -> Result<Instruction, ProgramError> {
    let mut accounts = vec![
        AccountMeta::new_readonly(*mango_group_pk, false),
        AccountMeta::new(*mango_cache_pk, false),
        AccountMeta::new(*root_bank_pk, false),
    ];

    accounts.extend(node_bank_pks.iter().map(|pk| AccountMeta::new_readonly(*pk, false)));

    let instr = MangoInstruction::UpdateRootBank;
    let data = instr.pack();
    Ok(Instruction { program_id: *program_id, accounts, data })
}

pub fn set_oracle(
    program_id: &Pubkey,
    mango_group_pk: &Pubkey,
    oracle_pk: &Pubkey,
    admin_pk: &Pubkey,
    price: I80F48,
) -> Result<Instruction, ProgramError> {
    let accounts = vec![
        AccountMeta::new_readonly(*mango_group_pk, false),
        AccountMeta::new(*oracle_pk, false),
        AccountMeta::new_readonly(*admin_pk, true),
    ];

    let instr = MangoInstruction::SetOracle { price };
    let data = instr.pack();
    Ok(Instruction { program_id: *program_id, accounts, data })
}

pub fn liquidate_token_and_token(
    program_id: &Pubkey,
    mango_group_pk: &Pubkey,
    mango_cache_pk: &Pubkey,
    liqee_mango_account_pk: &Pubkey,
    liqor_mango_account_pk: &Pubkey,
    liqor_pk: &Pubkey,
    asset_root_bank_pk: &Pubkey,
    asset_node_bank_pk: &Pubkey,
    liab_root_bank_pk: &Pubkey,
    liab_node_bank_pk: &Pubkey,
    liqee_open_orders_pks: &[Pubkey],
    liqor_open_orders_pks: &[Pubkey],
    max_liab_transfer: I80F48,
) -> Result<Instruction, ProgramError> {
    let mut accounts = vec![
        AccountMeta::new_readonly(*mango_group_pk, false),
        AccountMeta::new_readonly(*mango_cache_pk, false),
        AccountMeta::new(*liqee_mango_account_pk, false),
        AccountMeta::new(*liqor_mango_account_pk, false),
        AccountMeta::new_readonly(*liqor_pk, true),
        AccountMeta::new_readonly(*asset_root_bank_pk, false),
        AccountMeta::new(*asset_node_bank_pk, false),
        AccountMeta::new_readonly(*liab_root_bank_pk, false),
        AccountMeta::new(*liab_node_bank_pk, false),
    ];

    accounts.extend(liqee_open_orders_pks.iter().map(|pk| AccountMeta::new_readonly(*pk, false)));
    accounts.extend(liqor_open_orders_pks.iter().map(|pk| AccountMeta::new_readonly(*pk, false)));

    let instr = MangoInstruction::LiquidateTokenAndToken { max_liab_transfer };
    let data = instr.pack();
    Ok(Instruction { program_id: *program_id, accounts, data })
}
