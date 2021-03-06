//! Unit tests for the margin protocol module.

#![cfg(test)]

use super::*;
use mock::*;

use core::num::NonZeroI128;
use frame_support::{assert_noop, assert_ok};
use primitives::Leverage;
use sp_core::offchain::{
	testing::{TestOffchainExt, TestTransactionPoolExt},
	OffchainExt, TransactionPoolExt,
};
use sp_runtime::PerThing;

const EUR_JPY_PAIR: TradingPair = TradingPair {
	base: CurrencyId::FJPY,
	quote: CurrencyId::FEUR,
};

// `n` is a natural currency amount by cent, with 2 fractional digits precision
fn fixed128_from_natural_currency_cent(n: i128) -> Fixed128 {
	Fixed128::from_parts(n * 1_000_000_000_000_000_0)
}

// `b` is a natural currency amount by cent, with 2 fractional digits precision
fn balance_from_natural_currency_cent(b: u128) -> Balance {
	b * 1_000_000_000_000_000_0
}

fn risk_threshold(margin_call_percent: u32, stop_out_percent: u32) -> RiskThreshold {
	RiskThreshold {
		margin_call: Permill::from_percent(margin_call_percent),
		stop_out: Permill::from_percent(stop_out_percent),
	}
}

fn eur_jpy_long() -> Position<Runtime> {
	Position {
		owner: ALICE,
		pool: MOCK_POOL,
		pair: EUR_JPY_PAIR,
		leverage: Leverage::LongTwenty,
		leveraged_held: Fixed128::from_natural(100_000),
		leveraged_debits: Fixed128::from_natural(-14_104_090),
		leveraged_debits_in_usd: fixed128_from_natural_currency_cent(-131_813_93),
		open_accumulated_swap_rate: Fixed128::from_natural(1),
		open_margin: balance_from_natural_currency_cent(6_591_00),
	}
}

fn eur_jpy_short() -> Position<Runtime> {
	Position {
		owner: ALICE,
		pool: MOCK_POOL,
		pair: EUR_JPY_PAIR,
		leverage: Leverage::ShortTwenty,
		leveraged_held: Fixed128::from_natural(-100_000),
		leveraged_debits: Fixed128::from_natural(14_175_810),
		leveraged_debits_in_usd: fixed128_from_natural_currency_cent(133_734_06),
		open_accumulated_swap_rate: Fixed128::from_natural(1),
		open_margin: balance_from_natural_currency_cent(6_687_00),
	}
}

#[test]
fn unrealized_pl_of_long_position_works() {
	ExtBuilder::default()
		// USD/JPY = 110
		.price(CurrencyId::FJPY, (1, 110))
		// EUR/JPY = 140 => EUR/USD = 140/110
		.price(CurrencyId::FEUR, (140, 110))
		.build()
		.execute_with(|| {
			assert_eq!(
				MarginProtocol::_unrealized_pl_of_position(&eur_jpy_long()),
				Ok(Fixed128::from_parts(-1073545454545441750827)),
			);
		});
}

#[test]
fn unrealized_pl_of_short_position_works() {
	ExtBuilder::default()
		// USD/JPY = 110
		.price(CurrencyId::FJPY, (1, 110))
		// EUR/JPY = 140 => EUR/USD = 140/110
		.price(CurrencyId::FEUR, (140, 110))
		.build()
		.execute_with(|| {
			assert_eq!(
				MarginProtocol::_unrealized_pl_of_position(&eur_jpy_short()),
				Ok(Fixed128::from_parts(1470999999999987141081)),
			);
		});
}

#[test]
fn unrealized_pl_of_trader_sums_all_positions() {
	ExtBuilder::default()
		// USD/JPY = 110
		.price(CurrencyId::FJPY, (1, 110))
		// EUR/JPY = 140 => EUR/USD = 140/110
		.price(CurrencyId::FEUR, (140, 110))
		.build()
		.execute_with(|| {
			<Positions<Runtime>>::insert(0, eur_jpy_long());
			<Positions<Runtime>>::insert(1, eur_jpy_short());
			<PositionsByTrader<Runtime>>::insert(ALICE, MOCK_POOL, vec![0, 1]);
			assert_eq!(
				MarginProtocol::_unrealized_pl_of_trader(&ALICE),
				Ok(Fixed128::from_parts(397454545454545390254))
			);
		});
}

#[test]
fn margin_held_sums_all_open_margin() {
	ExtBuilder::default().build().execute_with(|| {
		<Positions<Runtime>>::insert(0, eur_jpy_long());
		<Positions<Runtime>>::insert(1, eur_jpy_short());
		<PositionsByTrader<Runtime>>::insert(ALICE, MOCK_POOL, vec![0, 1]);
		assert_eq!(
			MarginProtocol::_margin_held(&ALICE),
			balance_from_natural_currency_cent(13_278_00)
		);
	});
}

#[test]
fn free_balance_equal_to_balance_sub_margin_held() {
	ExtBuilder::default().build().execute_with(|| {
		<Balances<Runtime>>::insert(ALICE, balance_from_natural_currency_cent(15_000_00));
		<Positions<Runtime>>::insert(0, eur_jpy_long());
		<Positions<Runtime>>::insert(1, eur_jpy_short());
		<PositionsByTrader<Runtime>>::insert(ALICE, MOCK_POOL, vec![0, 1]);
		assert_eq!(
			MarginProtocol::_free_balance(&ALICE),
			balance_from_natural_currency_cent(1_722_00)
		);
	});
}

#[test]
fn free_balance_is_zero_if_margin_held_bigger_than_balance() {
	ExtBuilder::default().build().execute_with(|| {
		<Balances<Runtime>>::insert(ALICE, balance_from_natural_currency_cent(10_000_00));
		<Positions<Runtime>>::insert(0, eur_jpy_long());
		<Positions<Runtime>>::insert(1, eur_jpy_short());
		<PositionsByTrader<Runtime>>::insert(ALICE, MOCK_POOL, vec![0, 1]);
		assert_eq!(MarginProtocol::_free_balance(&ALICE), 0,);
	});
}

const EUR_USD_PAIR: TradingPair = TradingPair {
	base: CurrencyId::AUSD,
	quote: CurrencyId::FEUR,
};

fn eur_usd_long_1() -> Position<Runtime> {
	let open_rate =
		Fixed128::from_natural(1).saturating_add(Fixed128::from_rational(36_87, NonZeroI128::new(100_000_00).unwrap()));
	Position {
		owner: ALICE,
		pool: MOCK_POOL,
		pair: EUR_USD_PAIR,
		leverage: Leverage::LongFive,
		leveraged_held: Fixed128::from_natural(100_000),
		leveraged_debits: fixed128_from_natural_currency_cent(-120_420_30),
		leveraged_debits_in_usd: fixed128_from_natural_currency_cent(-120_420_30),
		open_accumulated_swap_rate: open_rate,
		open_margin: balance_from_natural_currency_cent(24_084_00),
	}
}

fn eur_usd_long_2() -> Position<Runtime> {
	let open_rate =
		Fixed128::from_natural(1).saturating_add(Fixed128::from_rational(18_43, NonZeroI128::new(100_000_00).unwrap()));
	Position {
		owner: ALICE,
		pool: MOCK_POOL,
		pair: EUR_USD_PAIR,
		leverage: Leverage::LongTwenty,
		leveraged_held: Fixed128::from_natural(100_000),
		leveraged_debits: fixed128_from_natural_currency_cent(-119_419_30),
		leveraged_debits_in_usd: fixed128_from_natural_currency_cent(-119_419_30),
		open_accumulated_swap_rate: open_rate,
		open_margin: balance_from_natural_currency_cent(5_971_00),
	}
}

fn eur_usd_short_1() -> Position<Runtime> {
	let open_rate =
		Fixed128::from_natural(1).saturating_sub(Fixed128::from_rational(10_96, NonZeroI128::new(100_000_00).unwrap()));
	Position {
		owner: ALICE,
		pool: MOCK_POOL,
		pair: EUR_USD_PAIR,
		leverage: Leverage::ShortTen,
		leveraged_held: Fixed128::from_natural(-100_000),
		leveraged_debits: fixed128_from_natural_currency_cent(119_780_10),
		leveraged_debits_in_usd: fixed128_from_natural_currency_cent(119_780_10),
		open_accumulated_swap_rate: open_rate,
		open_margin: balance_from_natural_currency_cent(11_978_00),
	}
}

fn eur_usd_short_2() -> Position<Runtime> {
	let open_rate =
		Fixed128::from_natural(1).saturating_sub(Fixed128::from_rational(3_65, NonZeroI128::new(100_000_00).unwrap()));
	Position {
		owner: ALICE,
		pool: MOCK_POOL,
		pair: EUR_USD_PAIR,
		leverage: Leverage::ShortFifty,
		leveraged_held: Fixed128::from_natural(-200_000),
		leveraged_debits: fixed128_from_natural_currency_cent(237_362_40),
		leveraged_debits_in_usd: fixed128_from_natural_currency_cent(237_362_40),
		open_accumulated_swap_rate: open_rate,
		open_margin: balance_from_natural_currency_cent(4_747_00),
	}
}

#[test]
fn accumulated_swap_rate_of_long_position_works() {
	ExtBuilder::default()
		.accumulated_swap_rate(EUR_USD_PAIR, Fixed128::from_natural(1))
		.build()
		.execute_with(|| {
			assert_eq!(
				MarginProtocol::_accumulated_swap_rate_of_position(&eur_usd_long_1()),
				Ok(fixed128_from_natural_currency_cent(-36_87))
			);
		});
}

#[test]
fn accumulated_swap_rate_of_short_position_works() {
	ExtBuilder::default()
		.accumulated_swap_rate(EUR_USD_PAIR, Fixed128::from_natural(1))
		.build()
		.execute_with(|| {
			assert_eq!(
				MarginProtocol::_accumulated_swap_rate_of_position(&eur_usd_short_1()),
				Ok(fixed128_from_natural_currency_cent(10_96))
			);
		});
}

#[test]
fn accumulated_swap_rate_of_trader_sums_all_positions() {
	ExtBuilder::default()
		.accumulated_swap_rate(EUR_USD_PAIR, Fixed128::from_natural(1))
		.build()
		.execute_with(|| {
			<Positions<Runtime>>::insert(0, eur_usd_long_1());
			<Positions<Runtime>>::insert(1, eur_usd_short_1());
			<PositionsByTrader<Runtime>>::insert(ALICE, MOCK_POOL, vec![0, 1]);
			assert_eq!(
				MarginProtocol::_accumulated_swap_rate_of_trader(&ALICE),
				Ok(fixed128_from_natural_currency_cent(-25_91))
			);
		});
}

#[test]
fn equity_of_trader_works() {
	ExtBuilder::default()
		.price(CurrencyId::FEUR, (12, 10))
		.accumulated_swap_rate(EUR_USD_PAIR, Fixed128::from_natural(1))
		.build()
		.execute_with(|| {
			<Balances<Runtime>>::insert(ALICE, balance_from_natural_currency_cent(120_000_00));
			<Positions<Runtime>>::insert(0, eur_usd_long_1());
			<Positions<Runtime>>::insert(1, eur_usd_long_2());
			<Positions<Runtime>>::insert(2, eur_usd_short_1());
			<Positions<Runtime>>::insert(3, eur_usd_short_2());
			<PositionsByTrader<Runtime>>::insert(ALICE, MOCK_POOL, vec![0, 1, 2, 3]);
			assert_eq!(
				MarginProtocol::_equity_of_trader(&ALICE),
				Ok(fixed128_from_natural_currency_cent(116_665_86))
			);
		});
}

#[test]
fn margin_level_works() {
	ExtBuilder::default()
		.price(CurrencyId::FEUR, (12, 10))
		.accumulated_swap_rate(EUR_USD_PAIR, Fixed128::from_natural(1))
		.build()
		.execute_with(|| {
			<Balances<Runtime>>::insert(ALICE, balance_from_natural_currency_cent(120_000_00));
			<Positions<Runtime>>::insert(0, eur_usd_long_1());
			<Positions<Runtime>>::insert(1, eur_usd_long_2());
			<Positions<Runtime>>::insert(2, eur_usd_short_1());
			<Positions<Runtime>>::insert(3, eur_usd_short_2());
			<PositionsByTrader<Runtime>>::insert(ALICE, MOCK_POOL, vec![0, 1, 2, 3]);
			assert_eq!(
				MarginProtocol::_margin_level(&ALICE, None, None),
				// 19.44%
				Ok(Fixed128::from_parts(195426060513372176))
			);
		});
}

#[test]
fn margin_level_with_new_position_works() {
	ExtBuilder::default().build().execute_with(|| {
		<Balances<Runtime>>::insert(ALICE, balance_from_natural_currency_cent(120_000_00));
		assert_eq!(
			MarginProtocol::_margin_level(&ALICE, Some(eur_usd_long_1()), None),
			// 120_000_00 / 120_420_30 = 0.996509724689275811
			Ok(Fixed128::from_parts(996509724689275811))
		);
	});
}

#[test]
fn margin_level_with_equity_delta_works() {
	ExtBuilder::default()
		.price(CurrencyId::FEUR, (12, 10))
		.accumulated_swap_rate(EUR_USD_PAIR, Fixed128::from_natural(1))
		.build()
		.execute_with(|| {
			<Balances<Runtime>>::insert(ALICE, balance_from_natural_currency_cent(120_000_00));
			<Positions<Runtime>>::insert(0, eur_usd_long_1());
			<Positions<Runtime>>::insert(1, eur_usd_long_2());
			<Positions<Runtime>>::insert(2, eur_usd_short_1());
			<Positions<Runtime>>::insert(3, eur_usd_short_2());
			<PositionsByTrader<Runtime>>::insert(ALICE, MOCK_POOL, vec![0, 1, 2, 3]);

			// positive delta
			assert_eq!(
				MarginProtocol::_margin_level(&ALICE, None, Some(fixed128_from_natural_currency_cent(10_000_00))),
				// 21.21%
				Ok(Fixed128::from_parts(212176981520886472))
			);

			// negative delta
			assert_eq!(
				MarginProtocol::_margin_level(&ALICE, None, Some(fixed128_from_natural_currency_cent(-10_000_00))),
				// 17.87%
				Ok(Fixed128::from_parts(178675139505857880))
			);
		});
}

#[test]
fn margin_level_without_any_opened_position_is_max() {
	ExtBuilder::default().build().execute_with(|| {
		<Balances<Runtime>>::insert(ALICE, 1);
		assert_eq!(
			MarginProtocol::_margin_level(&ALICE, None, None),
			Ok(Fixed128::max_value())
		);
	});
}

#[test]
fn ensure_trader_safe_works() {
	ExtBuilder::default()
		.spread(Permill::zero())
		.accumulated_swap_rate(EUR_USD_PAIR, Fixed128::from_natural(1))
		.price(CurrencyId::FEUR, (1, 1))
		.trader_risk_threshold(risk_threshold(100, 0))
		.build()
		.execute_with(|| {
			<Balances<Runtime>>::insert(ALICE, balance_from_natural_currency_cent(100));
			let position: Position<Runtime> = Position {
				owner: ALICE,
				pool: MOCK_POOL,
				pair: EUR_USD_PAIR,
				leverage: Leverage::LongTwo,
				leveraged_held: fixed128_from_natural_currency_cent(100),
				leveraged_debits: fixed128_from_natural_currency_cent(100),
				leveraged_debits_in_usd: fixed128_from_natural_currency_cent(100),
				open_accumulated_swap_rate: Fixed128::from_natural(1),
				open_margin: balance_from_natural_currency_cent(100),
			};
			assert_eq!(
				MarginProtocol::_margin_level(&ALICE, Some(position.clone()), None),
				Ok(Fixed128::from_natural(1))
			);

			// with new position

			// 100% == 100%, unsafe
			assert_noop!(
				MarginProtocol::_ensure_trader_safe(&ALICE, Some(position.clone()), None),
				Error::<Runtime>::TraderWouldBeUnsafe
			);
			// 100% > 99%, safe
			TraderRiskThreshold::put(risk_threshold(99, 0));
			assert_ok!(MarginProtocol::_ensure_trader_safe(
				&ALICE,
				Some(position.clone()),
				None
			));

			<Positions<Runtime>>::insert(0, position);
			<PositionsByTrader<Runtime>>::insert(ALICE, MOCK_POOL, vec![0]);
			assert_eq!(
				MarginProtocol::_margin_level(&ALICE, None, None),
				Ok(Fixed128::from_natural(1))
			);

			// without new position

			TraderRiskThreshold::put(risk_threshold(100, 0));

			// 100% == 100%, unsafe
			assert_noop!(
				MarginProtocol::_ensure_trader_safe(&ALICE, None, None),
				Error::<Runtime>::UnsafeTrader
			);
			// 100% > 99%, safe
			TraderRiskThreshold::put(risk_threshold(99, 0));
			assert_ok!(MarginProtocol::_ensure_trader_safe(&ALICE, None, None));

			// with equity delta

			// positive delta
			assert_ok!(MarginProtocol::_ensure_trader_safe(
				&ALICE,
				None,
				Some(fixed128_from_natural_currency_cent(10))
			));
			// negative delta
			assert_noop!(
				MarginProtocol::_ensure_trader_safe(&ALICE, None, Some(fixed128_from_natural_currency_cent(-10))),
				Error::<Runtime>::TraderWouldBeUnsafe
			);
		});
}

#[test]
fn equity_of_pool_works() {
	ExtBuilder::default()
		.pool_liquidity(MOCK_POOL, balance_from_natural_currency_cent(100_000_00))
		.price(CurrencyId::FEUR, (12, 10))
		.accumulated_swap_rate(EUR_USD_PAIR, Fixed128::from_natural(1))
		.build()
		.execute_with(|| {
			<Positions<Runtime>>::insert(0, eur_usd_long_1());
			<Positions<Runtime>>::insert(1, eur_usd_long_2());
			<Positions<Runtime>>::insert(2, eur_usd_short_1());
			<Positions<Runtime>>::insert(3, eur_usd_short_2());
			PositionsByPool::insert(MOCK_POOL, EUR_USD_PAIR, vec![0, 1, 2, 3]);
			assert_eq!(
				MarginProtocol::_equity_of_pool(MOCK_POOL),
				Ok(fixed128_from_natural_currency_cent(103_334_14))
			);
		});
}

#[test]
fn enp_and_ell_without_new_position_works() {
	ExtBuilder::default()
		.pool_liquidity(MOCK_POOL, balance_from_natural_currency_cent(100_000_00))
		.price(CurrencyId::FEUR, (12, 10))
		.accumulated_swap_rate(EUR_USD_PAIR, Fixed128::from_natural(1))
		.build()
		.execute_with(|| {
			<Positions<Runtime>>::insert(0, eur_usd_long_1());
			<Positions<Runtime>>::insert(1, eur_usd_long_2());
			<Positions<Runtime>>::insert(2, eur_usd_short_1());
			<Positions<Runtime>>::insert(3, eur_usd_short_2());
			PositionsByPool::insert(MOCK_POOL, EUR_USD_PAIR, vec![0, 1, 2, 3]);

			assert_eq!(
				MarginProtocol::_enp_and_ell(MOCK_POOL, None, None),
				Ok((
					Fixed128::from_parts(880917181075659681),
					Fixed128::from_parts(289335881335881335)
				))
			);
		});
}

#[test]
fn enp_and_ell_with_new_position_works() {
	ExtBuilder::default()
		.pool_liquidity(MOCK_POOL, balance_from_natural_currency_cent(100_000_00))
		.build()
		.execute_with(|| {
			// enp = ell = 100_000_00 / 120_420_30
			assert_eq!(
				MarginProtocol::_enp_and_ell(MOCK_POOL, Some(eur_usd_long_1()), None),
				Ok((
					Fixed128::from_parts(830424770574396509),
					Fixed128::from_parts(830424770574396509)
				))
			);
		});
}

#[test]
fn enp_and_ell_without_position_with_liquidity_works() {
	ExtBuilder::default()
		.pool_liquidity(MOCK_POOL, balance_from_natural_currency_cent(100_000_00))
		.price(CurrencyId::FEUR, (12, 10))
		.accumulated_swap_rate(EUR_USD_PAIR, Fixed128::from_natural(1))
		.build()
		.execute_with(|| {
			<Positions<Runtime>>::insert(0, eur_usd_long_1());
			<Positions<Runtime>>::insert(1, eur_usd_long_2());
			<Positions<Runtime>>::insert(2, eur_usd_short_1());
			<Positions<Runtime>>::insert(3, eur_usd_short_2());
			PositionsByPool::insert(MOCK_POOL, EUR_USD_PAIR, vec![0, 1, 2, 3]);

			assert_eq!(
				MarginProtocol::_enp_and_ell(MOCK_POOL, None, Some(fixed128_from_natural_currency_cent(-10))),
				Ok((
					Fixed128::from_parts(880916328581816817),
					Fixed128::from_parts(289335601335601335)
				))
			);
		});
}

#[test]
fn enp_and_ell_with_liquidity_works() {
	ExtBuilder::default()
		.pool_liquidity(MOCK_POOL, balance_from_natural_currency_cent(100_000_00))
		.build()
		.execute_with(|| {
			// enp = ell = (100_000_00 - 10)/ 120_420_30
			assert_eq!(
				MarginProtocol::_enp_and_ell(
					MOCK_POOL,
					Some(eur_usd_long_1()),
					Some(fixed128_from_natural_currency_cent(-10))
				),
				Ok((
					Fixed128::from_parts(830423940149625935),
					Fixed128::from_parts(830423940149625935)
				))
			);
		});
}

#[test]
fn ensure_liquidity_works() {
	ExtBuilder::default()
		.spread(Permill::zero())
		.accumulated_swap_rate(EUR_USD_PAIR, Fixed128::from_natural(1))
		.price(CurrencyId::FEUR, (1, 1))
		.pool_liquidity(MOCK_POOL, balance_from_natural_currency_cent(100))
		.liquidity_pool_ell_threshold(risk_threshold(99, 0))
		.liquidity_pool_enp_threshold(risk_threshold(99, 0))
		.build()
		.execute_with(|| {
			let position: Position<Runtime> = Position {
				owner: ALICE,
				pool: MOCK_POOL,
				pair: EUR_USD_PAIR,
				leverage: Leverage::LongTwo,
				leveraged_held: fixed128_from_natural_currency_cent(100),
				leveraged_debits: fixed128_from_natural_currency_cent(100),
				leveraged_debits_in_usd: fixed128_from_natural_currency_cent(100),
				open_accumulated_swap_rate: Fixed128::from_natural(1),
				open_margin: balance_from_natural_currency_cent(100),
			};

			<Positions<Runtime>>::insert(0, position);
			PositionsByPool::insert(MOCK_POOL, EUR_USD_PAIR, vec![0]);

			assert_ok!(MarginProtocol::ensure_can_withdraw(MOCK_POOL, 10));

			LiquidityPoolELLThreshold::put(risk_threshold(100, 0));

			assert_noop!(
				MarginProtocol::ensure_can_withdraw(MOCK_POOL, 1),
				Error::<Runtime>::PoolWouldBeUnsafe
			);
		});
}

#[test]
fn ensure_pool_safe_works() {
	ExtBuilder::default()
		.spread(Permill::zero())
		.accumulated_swap_rate(EUR_USD_PAIR, Fixed128::from_natural(1))
		.price(CurrencyId::FEUR, (1, 1))
		.pool_liquidity(MOCK_POOL, balance_from_natural_currency_cent(100))
		.liquidity_pool_ell_threshold(risk_threshold(99, 0))
		.liquidity_pool_enp_threshold(risk_threshold(99, 0))
		.build()
		.execute_with(|| {
			let position: Position<Runtime> = Position {
				owner: ALICE,
				pool: MOCK_POOL,
				pair: EUR_USD_PAIR,
				leverage: Leverage::LongTwo,
				leveraged_held: fixed128_from_natural_currency_cent(100),
				leveraged_debits: fixed128_from_natural_currency_cent(100),
				leveraged_debits_in_usd: fixed128_from_natural_currency_cent(100),
				open_accumulated_swap_rate: Fixed128::from_natural(1),
				open_margin: balance_from_natural_currency_cent(100),
			};

			// with new position
			assert_eq!(
				MarginProtocol::_enp_and_ell(MOCK_POOL, Some(position.clone()), None),
				Ok((Fixed128::from_natural(1), Fixed128::from_natural(1)))
			);

			// ENP 100% > 99%, ELL 100% > 99%, safe
			assert_ok!(MarginProtocol::_ensure_pool_safe(
				MOCK_POOL,
				Some(position.clone()),
				None
			));

			// ENP 100% == 100%, unsafe
			LiquidityPoolENPThreshold::put(risk_threshold(100, 0));
			assert_noop!(
				MarginProtocol::_ensure_pool_safe(MOCK_POOL, Some(position.clone()), None),
				Error::<Runtime>::PoolWouldBeUnsafe
			);

			// ELL 100% == 100%, unsafe
			LiquidityPoolENPThreshold::put(risk_threshold(99, 0));
			LiquidityPoolELLThreshold::put(risk_threshold(100, 0));
			assert_noop!(
				MarginProtocol::_ensure_pool_safe(MOCK_POOL, Some(position.clone()), None),
				Error::<Runtime>::PoolWouldBeUnsafe
			);

			// without new position
			<Positions<Runtime>>::insert(0, position);
			PositionsByPool::insert(MOCK_POOL, EUR_USD_PAIR, vec![0]);
			LiquidityPoolELLThreshold::put(risk_threshold(99, 0));
			assert_eq!(
				MarginProtocol::_enp_and_ell(MOCK_POOL, None, None),
				Ok((Fixed128::from_natural(1), Fixed128::from_natural(1)))
			);

			// ENP 100% > 99%, ELL 100% > 99%, safe
			assert_ok!(MarginProtocol::_ensure_pool_safe(MOCK_POOL, None, None));

			// ENP 100% == 100%, unsafe
			LiquidityPoolENPThreshold::put(risk_threshold(100, 0));
			assert_noop!(
				MarginProtocol::_ensure_pool_safe(MOCK_POOL, None, None),
				Error::<Runtime>::UnsafePool
			);

			// ELL 100% == 100%, unsafe
			LiquidityPoolENPThreshold::put(risk_threshold(99, 0));
			LiquidityPoolELLThreshold::put(risk_threshold(100, 0));
			assert_noop!(
				MarginProtocol::_ensure_pool_safe(MOCK_POOL, None, None),
				Error::<Runtime>::UnsafePool
			);
		});
}

#[test]
fn trader_margin_call_should_work() {
	ExtBuilder::default()
		.spread(Permill::zero())
		.accumulated_swap_rate(EUR_USD_PAIR, Fixed128::from_natural(1))
		.price(CurrencyId::FEUR, (1, 1))
		.trader_risk_threshold(risk_threshold(5, 3))
		.build()
		.execute_with(|| {
			<Balances<Runtime>>::insert(ALICE, balance_from_natural_currency_cent(100));
			let position: Position<Runtime> = Position {
				owner: ALICE,
				pool: MOCK_POOL,
				pair: EUR_USD_PAIR,
				leverage: Leverage::LongTwo,
				leveraged_held: fixed128_from_natural_currency_cent(100),
				leveraged_debits: fixed128_from_natural_currency_cent(100),
				leveraged_debits_in_usd: fixed128_from_natural_currency_cent(100),
				open_accumulated_swap_rate: Fixed128::from_natural(1),
				open_margin: balance_from_natural_currency_cent(100),
			};

			assert_eq!(
				MarginProtocol::_margin_level(&ALICE, None, None),
				Ok(Fixed128::max_value())
			);

			// without position
			assert_noop!(
				MarginProtocol::trader_margin_call(Origin::NONE, ALICE),
				Error::<Runtime>::SafeTrader
			);

			<Positions<Runtime>>::insert(0, position);
			<PositionsByTrader<Runtime>>::insert(ALICE, MOCK_POOL, vec![0]);
			assert_eq!(
				MarginProtocol::_margin_level(&ALICE, None, None),
				Ok(Fixed128::from_natural(1))
			);

			MockPrices::set_mock_price(CurrencyId::FEUR, Some(FixedU128::from_rational(1, 20)));
			assert_eq!(
				MarginProtocol::_margin_level(&ALICE, None, None),
				Ok(Fixed128::from_rational(5, NonZeroI128::new(100).unwrap()))
			);

			assert_ok!(MarginProtocol::trader_margin_call(Origin::NONE, ALICE));
		});
}

#[test]
fn trader_become_safe_should_work() {
	ExtBuilder::default()
		.spread(Permill::zero())
		.accumulated_swap_rate(EUR_USD_PAIR, Fixed128::from_natural(1))
		.price(CurrencyId::FEUR, (1, 1))
		.trader_risk_threshold(risk_threshold(5, 3))
		.build()
		.execute_with(|| {
			<Balances<Runtime>>::insert(ALICE, balance_from_natural_currency_cent(100));
			let position: Position<Runtime> = Position {
				owner: ALICE,
				pool: MOCK_POOL,
				pair: EUR_USD_PAIR,
				leverage: Leverage::LongTwo,
				leveraged_held: fixed128_from_natural_currency_cent(100),
				leveraged_debits: fixed128_from_natural_currency_cent(100),
				leveraged_debits_in_usd: fixed128_from_natural_currency_cent(100),
				open_accumulated_swap_rate: Fixed128::from_natural(1),
				open_margin: balance_from_natural_currency_cent(100),
			};

			// without position
			assert_ok!(MarginProtocol::trader_become_safe(Origin::NONE, ALICE));

			<Positions<Runtime>>::insert(0, position);
			<PositionsByTrader<Runtime>>::insert(ALICE, MOCK_POOL, vec![0]);
			assert_eq!(
				MarginProtocol::_margin_level(&ALICE, None, None),
				Ok(Fixed128::from_natural(1))
			);

			MockPrices::set_mock_price(CurrencyId::FEUR, Some(FixedU128::from_rational(4, 100)));
			assert_eq!(
				MarginProtocol::_margin_level(&ALICE, None, None),
				Ok(Fixed128::from_rational(4, NonZeroI128::new(100).unwrap()))
			);
			assert_ok!(MarginProtocol::trader_margin_call(Origin::NONE, ALICE));
			assert_noop!(
				MarginProtocol::trader_become_safe(Origin::NONE, ALICE),
				Error::<Runtime>::UnsafeTrader
			);

			MockPrices::set_mock_price(CurrencyId::FEUR, Some(FixedU128::from_rational(5, 100)));
			assert_eq!(
				MarginProtocol::_margin_level(&ALICE, None, None),
				Ok(Fixed128::from_rational(5, NonZeroI128::new(100).unwrap()))
			);
			assert_noop!(
				MarginProtocol::trader_become_safe(Origin::NONE, ALICE),
				Error::<Runtime>::UnsafeTrader
			);

			MockPrices::set_mock_price(CurrencyId::FEUR, Some(FixedU128::from_rational(6, 100)));
			assert_eq!(
				MarginProtocol::_margin_level(&ALICE, None, None),
				Ok(Fixed128::from_rational(6, NonZeroI128::new(100).unwrap()))
			);
			assert_ok!(MarginProtocol::trader_become_safe(Origin::NONE, ALICE));
		});
}
#[test]
fn trader_liquidate_should_work() {
	ExtBuilder::default()
		.spread(Permill::zero())
		.accumulated_swap_rate(EUR_USD_PAIR, Fixed128::from_natural(1))
		.price(CurrencyId::FEUR, (1, 1))
		.trader_risk_threshold(risk_threshold(5, 3))
		.build()
		.execute_with(|| {
			<Balances<Runtime>>::insert(ALICE, balance_from_natural_currency_cent(100));
			let position: Position<Runtime> = Position {
				owner: ALICE,
				pool: MOCK_POOL,
				pair: EUR_USD_PAIR,
				leverage: Leverage::LongTwo,
				leveraged_held: fixed128_from_natural_currency_cent(100),
				leveraged_debits: fixed128_from_natural_currency_cent(100),
				leveraged_debits_in_usd: fixed128_from_natural_currency_cent(100),
				open_accumulated_swap_rate: Fixed128::from_natural(1),
				open_margin: balance_from_natural_currency_cent(100),
			};

			// without position
			assert_noop!(
				MarginProtocol::trader_liquidate(Origin::NONE, ALICE),
				Error::<Runtime>::NotReachedRiskThreshold
			);

			<Positions<Runtime>>::insert(0, position);
			<PositionsByTrader<Runtime>>::insert(ALICE, MOCK_POOL, vec![0]);
			assert_eq!(
				MarginProtocol::_margin_level(&ALICE, None, None),
				Ok(Fixed128::from_natural(1))
			);

			// trader_liquidate without trader_margin_call
			MockPrices::set_mock_price(CurrencyId::FEUR, Some(FixedU128::from_rational(3, 100)));
			assert_eq!(
				MarginProtocol::_margin_level(&ALICE, None, None),
				Ok(Fixed128::from_rational(3, NonZeroI128::new(100).unwrap()))
			);
			assert_eq!(
				MarginProtocol::_free_balance(&ALICE),
				balance_from_natural_currency_cent(0)
			);

			assert_ok!(MarginProtocol::trader_liquidate(Origin::NONE, ALICE));
			assert_eq!(
				MarginProtocol::_free_balance(&ALICE),
				balance_from_natural_currency_cent(0)
			);
		});
}

#[test]
fn liquidity_pool_margin_call_and_become_safe_work() {
	ExtBuilder::default()
		.spread(Permill::zero())
		.accumulated_swap_rate(EUR_USD_PAIR, Fixed128::from_natural(1))
		.price(CurrencyId::FEUR, (1, 1))
		.pool_liquidity(MOCK_POOL, balance_from_natural_currency_cent(100))
		.liquidity_pool_ell_threshold(risk_threshold(99, 0))
		.liquidity_pool_enp_threshold(risk_threshold(99, 0))
		.build()
		.execute_with(|| {
			let position: Position<Runtime> = Position {
				owner: ALICE,
				pool: MOCK_POOL,
				pair: EUR_USD_PAIR,
				leverage: Leverage::LongTwo,
				leveraged_held: fixed128_from_natural_currency_cent(100),
				leveraged_debits: fixed128_from_natural_currency_cent(100),
				leveraged_debits_in_usd: fixed128_from_natural_currency_cent(100),
				open_accumulated_swap_rate: Fixed128::from_natural(1),
				open_margin: balance_from_natural_currency_cent(100),
			};

			<Positions<Runtime>>::insert(0, position);
			PositionsByPool::insert(MOCK_POOL, EUR_USD_PAIR, vec![0]);
			assert_eq!(
				MarginProtocol::_enp_and_ell(MOCK_POOL, None, None),
				Ok((Fixed128::from_natural(1), Fixed128::from_natural(1)))
			);

			// ENP 100% > 99%, ELL 100% > 99%, safe
			assert_noop!(
				MarginProtocol::liquidity_pool_margin_call(Origin::NONE, MOCK_POOL),
				Error::<Runtime>::SafePool
			);
			assert_ok!(MarginProtocol::liquidity_pool_become_safe(Origin::NONE, MOCK_POOL));

			// ENP 100% == 100%, unsafe
			LiquidityPoolENPThreshold::put(risk_threshold(100, 0));
			assert_ok!(MarginProtocol::liquidity_pool_margin_call(Origin::NONE, MOCK_POOL));
			let event = TestEvent::margin_protocol(RawEvent::LiquidityPoolMarginCalled(MOCK_POOL));
			assert!(System::events().iter().any(|record| record.event == event));

			assert_noop!(
				MarginProtocol::liquidity_pool_become_safe(Origin::NONE, MOCK_POOL),
				Error::<Runtime>::UnsafePool
			);

			// ENP 100% > 99%, ELL 100% > 99%, safe
			LiquidityPoolENPThreshold::put(risk_threshold(99, 0));
			assert_ok!(MarginProtocol::liquidity_pool_margin_call(Origin::NONE, MOCK_POOL));
			assert_ok!(MarginProtocol::liquidity_pool_become_safe(Origin::NONE, MOCK_POOL));
			let event = TestEvent::margin_protocol(RawEvent::LiquidityPoolBecameSafe(MOCK_POOL));
			assert!(System::events().iter().any(|record| record.event == event));
		});
}

#[test]
fn liquidity_pool_liquidate_works() {
	ExtBuilder::default()
		.spread(Permill::from_rational_approximation(1, 100u32))
		.accumulated_swap_rate(EUR_USD_PAIR, Fixed128::from_natural(1))
		.price(CurrencyId::FEUR, (1, 1))
		.pool_liquidity(MOCK_POOL, balance_from_natural_currency_cent(10_000_00))
		.liquidity_pool_ell_threshold(risk_threshold(0, 99))
		.liquidity_pool_enp_threshold(risk_threshold(0, 99))
		.build()
		.execute_with(|| {
			<Balances<Runtime>>::insert(ALICE, balance_from_natural_currency_cent(10_000_00));
			assert_ok!(MarginProtocol::open_position(
				Origin::signed(ALICE),
				MOCK_POOL,
				EUR_USD_PAIR,
				Leverage::LongTwenty,
				balance_from_natural_currency_cent(10_000_00),
				Price::from_natural(2)
			));

			assert_eq!(
				MarginProtocol::balances(ALICE),
				balance_from_natural_currency_cent(10_000_00)
			);

			// ENP 100% > 99%, ELL 100% > 99%, safe
			assert_noop!(
				MarginProtocol::liquidity_pool_liquidate(Origin::NONE, MOCK_POOL),
				Error::<Runtime>::NotReachedRiskThreshold
			);

			// Open position spread is 100
			// Current price is 20, close position spread is 200.
			// So liquidity remain 300. Total penalty is 200*2 = 400.
			MockPrices::set_mock_price(CurrencyId::FEUR, Some(FixedU128::from_rational(2, 1)));
			// ENP 50% < 99%, unsafe
			assert_ok!(MarginProtocol::liquidity_pool_liquidate(Origin::NONE, MOCK_POOL));

			let event = TestEvent::margin_protocol(RawEvent::LiquidityPoolLiquidated(MOCK_POOL));
			assert!(System::events().iter().any(|record| record.event == event));

			assert_eq!(
				MarginProtocol::balances(ALICE),
				balance_from_natural_currency_cent(19_700_00)
			);
			assert_eq!(
				MockLiquidityPools::liquidity(MOCK_POOL),
				balance_from_natural_currency_cent(0)
			);
			assert_eq!(
				OrmlTokens::total_balance(CurrencyId::AUSD, &TREASURY_ACCOUNT),
				balance_from_natural_currency_cent(300_00)
			);
		});
}

#[test]
fn open_long_position_works() {
	ExtBuilder::default()
		// USD/JPY = 107
		.price(CurrencyId::FJPY, (1, 107))
		// EUR/JPY = 140.9 => EUR/USD = 140.9/107
		.price(CurrencyId::FEUR, (1409, 1070))
		.accumulated_swap_rate(EUR_JPY_PAIR, Fixed128::from_natural(1))
		.pool_liquidity(MOCK_POOL, balance_from_natural_currency_cent(100_000_00))
		.build()
		.execute_with(|| {
			<Balances<Runtime>>::insert(ALICE, balance_from_natural_currency_cent(10_000_00));
			assert_ok!(MarginProtocol::open_position(
				Origin::signed(ALICE),
				MOCK_POOL,
				EUR_JPY_PAIR,
				Leverage::LongTwenty,
				balance_from_natural_currency_cent(100_000_00),
				Price::from_natural(142)
			));

			let position = {
				let mut p = eur_jpy_long();
				// with higher precision level
				p.leveraged_debits = Fixed128::from_parts(-14104090000000000732500000);
				p.leveraged_debits_in_usd = Fixed128::from_parts(-131813925233644859804554);
				p.open_margin = 6590696261682242990227;
				p
			};
			let id = 0;
			assert_eq!(MarginProtocol::positions(id), Some(position));
			assert_eq!(MarginProtocol::positions_by_trader(ALICE, MOCK_POOL), vec![id]);
			assert_eq!(MarginProtocol::positions_by_pool(MOCK_POOL, EUR_JPY_PAIR), vec![id]);
			assert_eq!(MarginProtocol::next_position_id(), 1);

			let event = TestEvent::margin_protocol(RawEvent::PositionOpened(
				ALICE,
				MOCK_POOL,
				EUR_JPY_PAIR,
				Leverage::LongTwenty,
				balance_from_natural_currency_cent(100_000_00),
				// price: 141.0409
				Price::from_parts(141040900000000007325),
			));
			assert!(System::events().iter().any(|record| record.event == event));
		});
}

#[test]
fn open_short_position_works() {
	ExtBuilder::default()
		// USD/JPY = 106
		.price(CurrencyId::FJPY, (1, 106))
		// EUR/JPY = 141.9 => EUR/USD = 141.9/106
		.price(CurrencyId::FEUR, (1419, 1060))
		.accumulated_swap_rate(EUR_JPY_PAIR, Fixed128::from_natural(1))
		.pool_liquidity(MOCK_POOL, balance_from_natural_currency_cent(100_000_00))
		.build()
		.execute_with(|| {
			<Balances<Runtime>>::insert(ALICE, balance_from_natural_currency_cent(10_000_00));
			assert_ok!(MarginProtocol::open_position(
				Origin::signed(ALICE),
				MOCK_POOL,
				EUR_JPY_PAIR,
				Leverage::ShortTwenty,
				balance_from_natural_currency_cent(100_000_00),
				Price::from_natural(141)
			));

			let position = {
				let mut p = eur_jpy_short();
				// with higher precision level
				p.leveraged_debits = Fixed128::from_parts(14175810000000000585500000);
				p.leveraged_debits_in_usd = Fixed128::from_parts(133734056603773584812414);
				p.open_margin = 6686702830188679240620;
				p
			};
			assert_eq!(MarginProtocol::positions(0), Some(position));
		});
}

#[test]
fn open_position_fails_if_trader_margin_called() {
	ExtBuilder::default()
		// USD/JPY = 107
		.price(CurrencyId::FJPY, (1, 107))
		// EUR/JPY = 140.9 => EUR/USD = 140.9/107
		.price(CurrencyId::FEUR, (1409, 1070))
		.accumulated_swap_rate(EUR_JPY_PAIR, Fixed128::from_natural(1))
		.pool_liquidity(MOCK_POOL, balance_from_natural_currency_cent(100_000_00))
		.build()
		.execute_with(|| {
			<Balances<Runtime>>::insert(ALICE, balance_from_natural_currency_cent(10_000_00));
			<MarginCalledTraders<Runtime>>::insert(ALICE, ());
			assert_noop!(
				MarginProtocol::open_position(
					Origin::signed(ALICE),
					MOCK_POOL,
					EUR_JPY_PAIR,
					Leverage::LongTwenty,
					balance_from_natural_currency_cent(100_000_00),
					Price::from_natural(142)
				),
				Error::<Runtime>::MarginCalledTrader
			);
		});
}

#[test]
fn open_position_fails_if_pool_margin_called() {
	ExtBuilder::default()
		// USD/JPY = 107
		.price(CurrencyId::FJPY, (1, 107))
		// EUR/JPY = 140.9 => EUR/USD = 140.9/107
		.price(CurrencyId::FEUR, (1409, 1070))
		.accumulated_swap_rate(EUR_JPY_PAIR, Fixed128::from_natural(1))
		.pool_liquidity(MOCK_POOL, balance_from_natural_currency_cent(100_000_00))
		.build()
		.execute_with(|| {
			<Balances<Runtime>>::insert(ALICE, balance_from_natural_currency_cent(10_000_00));
			MarginCalledPools::insert(MOCK_POOL, ());
			assert_noop!(
				MarginProtocol::open_position(
					Origin::signed(ALICE),
					MOCK_POOL,
					EUR_JPY_PAIR,
					Leverage::LongTwenty,
					balance_from_natural_currency_cent(100_000_00),
					Price::from_natural(142)
				),
				Error::<Runtime>::MarginCalledPool
			);
		});
}

#[test]
fn open_position_fails_if_no_base_price() {
	ExtBuilder::default()
		.price(CurrencyId::FEUR, (1409, 1070))
		.accumulated_swap_rate(EUR_JPY_PAIR, Fixed128::from_natural(1))
		.pool_liquidity(MOCK_POOL, balance_from_natural_currency_cent(100_000_00))
		.build()
		.execute_with(|| {
			<Balances<Runtime>>::insert(ALICE, balance_from_natural_currency_cent(10_000_00));
			assert_noop!(
				MarginProtocol::open_position(
					Origin::signed(ALICE),
					MOCK_POOL,
					EUR_JPY_PAIR,
					Leverage::LongTwenty,
					balance_from_natural_currency_cent(100_000_00),
					Price::from_natural(142)
				),
				Error::<Runtime>::NoPrice
			);
		});
}

#[test]
fn open_position_fails_if_no_quote_price() {
	ExtBuilder::default()
		.price(CurrencyId::FJPY, (1, 107))
		.accumulated_swap_rate(EUR_JPY_PAIR, Fixed128::from_natural(1))
		.pool_liquidity(MOCK_POOL, balance_from_natural_currency_cent(100_000_00))
		.build()
		.execute_with(|| {
			<Balances<Runtime>>::insert(ALICE, balance_from_natural_currency_cent(10_000_00));
			assert_noop!(
				MarginProtocol::open_position(
					Origin::signed(ALICE),
					MOCK_POOL,
					EUR_JPY_PAIR,
					Leverage::LongTwenty,
					balance_from_natural_currency_cent(100_000_00),
					Price::from_natural(142)
				),
				Error::<Runtime>::NoPrice
			);
		});
}

#[test]
fn open_long_position_fails_if_market_price_too_high() {
	ExtBuilder::default()
		// USD/JPY = 107
		.price(CurrencyId::FJPY, (1, 107))
		// EUR/JPY = 140.9 => EUR/USD = 140.9/107
		.price(CurrencyId::FEUR, (1409, 1070))
		.accumulated_swap_rate(EUR_JPY_PAIR, Fixed128::from_natural(1))
		.pool_liquidity(MOCK_POOL, balance_from_natural_currency_cent(100_000_00))
		.build()
		.execute_with(|| {
			<Balances<Runtime>>::insert(ALICE, balance_from_natural_currency_cent(10_000_00));
			assert_noop!(
				MarginProtocol::open_position(
					Origin::signed(ALICE),
					MOCK_POOL,
					EUR_JPY_PAIR,
					Leverage::LongTwenty,
					balance_from_natural_currency_cent(100_000_00),
					Price::from_natural(141)
				),
				Error::<Runtime>::MarketPriceTooHigh
			);
		});
}

#[test]
fn open_short_position_fails_if_market_price_too_low() {
	ExtBuilder::default()
		// USD/JPY = 106
		.price(CurrencyId::FJPY, (1, 106))
		// EUR/JPY = 141.9 => EUR/USD = 141.9/106
		.price(CurrencyId::FEUR, (1419, 1060))
		.accumulated_swap_rate(EUR_JPY_PAIR, Fixed128::from_natural(1))
		.pool_liquidity(MOCK_POOL, balance_from_natural_currency_cent(100_000_00))
		.build()
		.execute_with(|| {
			<Balances<Runtime>>::insert(ALICE, balance_from_natural_currency_cent(10_000_00));
			assert_noop!(
				MarginProtocol::open_position(
					Origin::signed(ALICE),
					MOCK_POOL,
					EUR_JPY_PAIR,
					Leverage::ShortTwenty,
					balance_from_natural_currency_cent(100_000_00),
					Price::from_natural(142)
				),
				Error::<Runtime>::MarketPriceTooLow
			);
		});
}

#[test]
fn open_position_fails_if_leveraged_debits_out_of_bound() {
	ExtBuilder::default()
		.price(CurrencyId::FJPY, (1, 1))
		.price(CurrencyId::FEUR, (2, 1))
		.accumulated_swap_rate(EUR_JPY_PAIR, Fixed128::from_natural(1))
		.pool_liquidity(MOCK_POOL, Balance::max_value())
		.build()
		.execute_with(|| {
			<Balances<Runtime>>::insert(ALICE, Balance::max_value());
			assert_noop!(
				MarginProtocol::open_position(
					Origin::signed(ALICE),
					MOCK_POOL,
					EUR_JPY_PAIR,
					Leverage::LongTwenty,
					Balance::max_value() / 2 + 1,
					Price::from_natural(142)
				),
				Error::<Runtime>::NumOutOfBound
			);
		});
}

#[test]
fn open_position_fails_if_trader_would_be_unsafe() {
	ExtBuilder::default()
		// USD/JPY = 107
		.price(CurrencyId::FJPY, (1, 107))
		// EUR/JPY = 140.9 => EUR/USD = 140.9/107
		.price(CurrencyId::FEUR, (1409, 1070))
		.accumulated_swap_rate(EUR_JPY_PAIR, Fixed128::from_natural(1))
		.pool_liquidity(MOCK_POOL, balance_from_natural_currency_cent(100_000_00))
		.trader_risk_threshold(risk_threshold(10, 5))
		.build()
		.execute_with(|| {
			<Balances<Runtime>>::insert(ALICE, balance_from_natural_currency_cent(659_00));
			assert_noop!(
				MarginProtocol::open_position(
					Origin::signed(ALICE),
					MOCK_POOL,
					EUR_JPY_PAIR,
					Leverage::LongTwenty,
					balance_from_natural_currency_cent(100_000_00),
					Price::from_natural(142)
				),
				Error::<Runtime>::TraderWouldBeUnsafe
			);
		});
}

#[test]
fn open_position_fails_if_would_reach_enp_threshold() {
	ExtBuilder::default()
		// USD/JPY = 107
		.price(CurrencyId::FJPY, (1, 107))
		// EUR/JPY = 140.9 => EUR/USD = 140.9/107
		.price(CurrencyId::FEUR, (1409, 1070))
		.accumulated_swap_rate(EUR_JPY_PAIR, Fixed128::from_natural(1))
		.pool_liquidity(MOCK_POOL, balance_from_natural_currency_cent(659_00))
		.liquidity_pool_enp_threshold(risk_threshold(10, 5))
		.build()
		.execute_with(|| {
			<Balances<Runtime>>::insert(ALICE, balance_from_natural_currency_cent(10_000_00));
			assert_noop!(
				MarginProtocol::open_position(
					Origin::signed(ALICE),
					MOCK_POOL,
					EUR_JPY_PAIR,
					Leverage::LongTwenty,
					balance_from_natural_currency_cent(100_000_00),
					Price::from_natural(142)
				),
				Error::<Runtime>::PoolWouldBeUnsafe
			);
		});
}

#[test]
fn open_position_fails_if_would_reach_ell_threshold() {
	ExtBuilder::default()
		// USD/JPY = 107
		.price(CurrencyId::FJPY, (1, 107))
		// EUR/JPY = 140.9 => EUR/USD = 140.9/107
		.price(CurrencyId::FEUR, (1409, 1070))
		.accumulated_swap_rate(EUR_JPY_PAIR, Fixed128::from_natural(1))
		.pool_liquidity(MOCK_POOL, balance_from_natural_currency_cent(659_00))
		.liquidity_pool_ell_threshold(risk_threshold(10, 5))
		.build()
		.execute_with(|| {
			<Balances<Runtime>>::insert(ALICE, balance_from_natural_currency_cent(10_000_00));
			assert_noop!(
				MarginProtocol::open_position(
					Origin::signed(ALICE),
					MOCK_POOL,
					EUR_JPY_PAIR,
					Leverage::LongTwenty,
					balance_from_natural_currency_cent(100_000_00),
					Price::from_natural(142)
				),
				Error::<Runtime>::PoolWouldBeUnsafe
			);
		});
}

#[test]
fn open_position_fails_if_run_out_of_position_id() {
	ExtBuilder::default()
		// USD/JPY = 107
		.price(CurrencyId::FJPY, (1, 107))
		// EUR/JPY = 140.9 => EUR/USD = 140.9/107
		.price(CurrencyId::FEUR, (1409, 1070))
		.accumulated_swap_rate(EUR_JPY_PAIR, Fixed128::from_natural(1))
		.pool_liquidity(MOCK_POOL, balance_from_natural_currency_cent(100_000_00))
		.build()
		.execute_with(|| {
			<Balances<Runtime>>::insert(ALICE, balance_from_natural_currency_cent(10_000_00));
			NextPositionId::put(PositionId::max_value());
			assert_noop!(
				MarginProtocol::open_position(
					Origin::signed(ALICE),
					MOCK_POOL,
					EUR_JPY_PAIR,
					Leverage::LongTwenty,
					balance_from_natural_currency_cent(100_000_00),
					Price::from_natural(142)
				),
				Error::<Runtime>::NoAvailablePositionId
			);
		});
}

#[test]
fn close_loss_position_works() {
	let alice_initial = balance_from_natural_currency_cent(10_000_00);
	ExtBuilder::default()
		.module_balance(alice_initial)
		// EUR/USD = 1.2
		.price(CurrencyId::FEUR, (12, 10))
		.accumulated_swap_rate(EUR_USD_PAIR, Fixed128::from_natural(1))
		.pool_liquidity(MOCK_POOL, balance_from_natural_currency_cent(100_000_00))
		.build()
		.execute_with(|| {
			<Balances<Runtime>>::insert(ALICE, alice_initial);

			let position = eur_usd_long_1();
			let id = 0;
			<Positions<Runtime>>::insert(id, position);
			<PositionsByTrader<Runtime>>::insert(ALICE, MOCK_POOL, vec![0]);
			PositionsByPool::insert(MOCK_POOL, EUR_USD_PAIR, vec![0]);

			assert_ok!(MarginProtocol::close_position(
				Origin::signed(ALICE),
				id,
				Price::from_rational(11, 10)
			));

			// realized math
			assert_eq!(
				MarginProtocol::balances(ALICE),
				balance_from_natural_currency_cent(9422_83)
			);
			assert_eq!(
				MockLiquidityPools::liquidity(MOCK_POOL),
				balance_from_natural_currency_cent(100577_17)
			);
			assert_eq!(
				OrmlTokens::free_balance(CurrencyId::AUSD, &MarginProtocol::account_id()),
				balance_from_natural_currency_cent(9422_83)
			);

			// position removed
			assert!(MarginProtocol::positions(id).is_none());
			assert_eq!(MarginProtocol::positions_by_trader(ALICE, MOCK_POOL), vec![]);
			assert_eq!(MarginProtocol::positions_by_pool(MOCK_POOL, EUR_USD_PAIR), vec![]);

			let event =
				TestEvent::margin_protocol(RawEvent::PositionClosed(ALICE, id, Price::from_rational(11988, 10000)));
			assert!(System::events().iter().any(|record| record.event == event));
		});
}

#[test]
fn close_profit_position_works() {
	let alice_initial = balance_from_natural_currency_cent(10_000_00);
	ExtBuilder::default()
		.module_balance(alice_initial)
		// EUR/USD = 1.2
		.price(CurrencyId::FEUR, (12, 10))
		.accumulated_swap_rate(EUR_USD_PAIR, Fixed128::from_natural(1))
		.pool_liquidity(MOCK_POOL, balance_from_natural_currency_cent(100_000_00))
		.build()
		.execute_with(|| {
			<Balances<Runtime>>::insert(ALICE, alice_initial);

			let position = eur_usd_long_2();
			let id = 0;
			<Positions<Runtime>>::insert(id, position);
			<PositionsByTrader<Runtime>>::insert(ALICE, MOCK_POOL, vec![0]);
			PositionsByPool::insert(MOCK_POOL, EUR_USD_PAIR, vec![0]);

			assert_ok!(MarginProtocol::close_position(
				Origin::signed(ALICE),
				id,
				Price::from_rational(11, 10)
			));

			assert_eq!(
				MarginProtocol::balances(ALICE),
				balance_from_natural_currency_cent(10442_27)
			);
			assert_eq!(
				MockLiquidityPools::liquidity(MOCK_POOL),
				balance_from_natural_currency_cent(99557_73)
			);
			assert_eq!(
				OrmlTokens::free_balance(CurrencyId::AUSD, &MarginProtocol::account_id()),
				balance_from_natural_currency_cent(10442_27)
			);
		});
}

#[test]
fn close_position_fails_if_position_not_found() {
	let alice_initial = balance_from_natural_currency_cent(10_000_00);
	ExtBuilder::default()
		.module_balance(alice_initial)
		// EUR/USD = 1.2
		.price(CurrencyId::FEUR, (12, 10))
		.accumulated_swap_rate(EUR_USD_PAIR, Fixed128::from_natural(1))
		.pool_liquidity(MOCK_POOL, balance_from_natural_currency_cent(100_000_00))
		.build()
		.execute_with(|| {
			<Balances<Runtime>>::insert(ALICE, alice_initial);

			assert_noop!(
				MarginProtocol::close_position(Origin::signed(ALICE), 0, Price::from_rational(11, 10)),
				Error::<Runtime>::PositionNotFound
			);
		});
}

#[test]
fn close_position_fails_if_position_not_opened_by_trader() {
	let alice_initial = balance_from_natural_currency_cent(10_000_00);
	ExtBuilder::default()
		.module_balance(alice_initial)
		// EUR/USD = 1.2
		.price(CurrencyId::FEUR, (12, 10))
		.accumulated_swap_rate(EUR_USD_PAIR, Fixed128::from_natural(1))
		.pool_liquidity(MOCK_POOL, balance_from_natural_currency_cent(100_000_00))
		.build()
		.execute_with(|| {
			<Balances<Runtime>>::insert(ALICE, alice_initial);

			let position = eur_usd_long_1();
			let id = 0;
			<Positions<Runtime>>::insert(id, position);
			<PositionsByTrader<Runtime>>::insert(ALICE, MOCK_POOL, vec![0]);
			PositionsByPool::insert(MOCK_POOL, EUR_USD_PAIR, vec![0]);

			assert_noop!(
				MarginProtocol::close_position(Origin::signed(BOB), 0, Price::from_rational(11, 10)),
				Error::<Runtime>::PositionNotOpenedByTrader
			);
		});
}

#[test]
fn close_position_fails_if_unrealized_out_of_bound() {
	let alice_initial = balance_from_natural_currency_cent(10_000_00);
	ExtBuilder::default()
		.module_balance(alice_initial)
		.price(CurrencyId::FEUR, (u128::max_value(), 1))
		.accumulated_swap_rate(EUR_USD_PAIR, Fixed128::from_natural(1))
		.pool_liquidity(MOCK_POOL, balance_from_natural_currency_cent(100_000_00))
		.build()
		.execute_with(|| {
			<Balances<Runtime>>::insert(ALICE, alice_initial);

			let position = eur_usd_long_1();
			let id = 0;
			<Positions<Runtime>>::insert(id, position);
			<PositionsByTrader<Runtime>>::insert(ALICE, MOCK_POOL, vec![0]);
			PositionsByPool::insert(MOCK_POOL, EUR_USD_PAIR, vec![0]);

			assert_noop!(
				MarginProtocol::close_position(Origin::signed(ALICE), 0, Price::from_rational(11, 10)),
				Error::<Runtime>::NumOutOfBound
			);
		});
}

#[test]
fn close_position_fails_if_no_base_price() {
	let alice_initial = balance_from_natural_currency_cent(10_000_00);
	ExtBuilder::default()
		.module_balance(alice_initial)
		.price(CurrencyId::FEUR, (1409, 1070))
		.accumulated_swap_rate(EUR_USD_PAIR, Fixed128::from_natural(1))
		.pool_liquidity(MOCK_POOL, balance_from_natural_currency_cent(100_000_00))
		.build()
		.execute_with(|| {
			<Balances<Runtime>>::insert(ALICE, alice_initial);

			let position = eur_jpy_long();
			let id = 0;
			<Positions<Runtime>>::insert(id, position);
			<PositionsByTrader<Runtime>>::insert(ALICE, MOCK_POOL, vec![0]);
			PositionsByPool::insert(MOCK_POOL, EUR_USD_PAIR, vec![0]);

			assert_noop!(
				MarginProtocol::close_position(Origin::signed(ALICE), 0, Price::from_rational(1410, 1070)),
				Error::<Runtime>::NoPrice
			);
		});
}

#[test]
fn close_position_fails_if_no_quote_price() {
	let alice_initial = balance_from_natural_currency_cent(10_000_00);
	ExtBuilder::default()
		.module_balance(alice_initial)
		.price(CurrencyId::FJPY, (1, 107))
		.accumulated_swap_rate(EUR_USD_PAIR, Fixed128::from_natural(1))
		.pool_liquidity(MOCK_POOL, balance_from_natural_currency_cent(100_000_00))
		.build()
		.execute_with(|| {
			<Balances<Runtime>>::insert(ALICE, alice_initial);

			let position = eur_jpy_long();
			let id = 0;
			<Positions<Runtime>>::insert(id, position);
			<PositionsByTrader<Runtime>>::insert(ALICE, MOCK_POOL, vec![0]);
			PositionsByPool::insert(MOCK_POOL, EUR_USD_PAIR, vec![0]);

			assert_noop!(
				MarginProtocol::close_position(Origin::signed(ALICE), 0, Price::from_rational(1390, 1070)),
				Error::<Runtime>::NoPrice
			);
		});
}

#[test]
fn close_long_position_fails_if_market_price_too_low() {
	let alice_initial = balance_from_natural_currency_cent(10_000_00);
	ExtBuilder::default()
		.module_balance(alice_initial)
		// EUR/USD = 1.2
		.price(CurrencyId::FEUR, (12, 10))
		.accumulated_swap_rate(EUR_USD_PAIR, Fixed128::from_natural(1))
		.pool_liquidity(MOCK_POOL, balance_from_natural_currency_cent(100_000_00))
		.build()
		.execute_with(|| {
			<Balances<Runtime>>::insert(ALICE, alice_initial);

			let position = eur_usd_long_1();
			let id = 0;
			<Positions<Runtime>>::insert(id, position);
			<PositionsByTrader<Runtime>>::insert(ALICE, MOCK_POOL, vec![0]);
			PositionsByPool::insert(MOCK_POOL, EUR_USD_PAIR, vec![0]);

			assert_noop!(
				MarginProtocol::close_position(Origin::signed(ALICE), 0, Price::from_rational(12, 10)),
				Error::<Runtime>::MarketPriceTooLow
			);
		});
}

#[test]
fn close_short_position_fails_if_market_price_too_high() {
	let alice_initial = balance_from_natural_currency_cent(10_000_00);
	ExtBuilder::default()
		.module_balance(alice_initial)
		// EUR/USD = 1.2
		.price(CurrencyId::FEUR, (12, 10))
		.accumulated_swap_rate(EUR_USD_PAIR, Fixed128::from_natural(1))
		.pool_liquidity(MOCK_POOL, balance_from_natural_currency_cent(100_000_00))
		.build()
		.execute_with(|| {
			<Balances<Runtime>>::insert(ALICE, alice_initial);

			let position = eur_usd_short_1();
			let id = 0;
			<Positions<Runtime>>::insert(id, position);
			<PositionsByTrader<Runtime>>::insert(ALICE, MOCK_POOL, vec![0]);
			PositionsByPool::insert(MOCK_POOL, EUR_USD_PAIR, vec![0]);

			assert_noop!(
				MarginProtocol::close_position(Origin::signed(ALICE), 0, Price::from_rational(12, 10)),
				Error::<Runtime>::MarketPriceTooHigh
			);
		});
}

#[test]
fn deposit_works() {
	ExtBuilder::default().alice_balance(1000).build().execute_with(|| {
		assert_eq!(OrmlTokens::free_balance(CurrencyId::AUSD, &ALICE), 1000);
		assert_eq!(
			OrmlTokens::free_balance(CurrencyId::AUSD, &MarginProtocol::account_id()),
			0
		);

		assert_ok!(MarginProtocol::deposit(Origin::signed(ALICE), 500));

		assert_eq!(OrmlTokens::free_balance(CurrencyId::AUSD, &ALICE), 500);
		assert_eq!(
			OrmlTokens::free_balance(CurrencyId::AUSD, &MarginProtocol::account_id()),
			500
		);

		let event = TestEvent::margin_protocol(RawEvent::Deposited(ALICE, 500));
		assert!(System::events().iter().any(|record| record.event == event));
	});
}

#[test]
fn deposit_fails_if_transfer_err() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			MarginProtocol::deposit(Origin::signed(ALICE), 500),
			orml_tokens::Error::<Runtime>::BalanceTooLow
		);
	});
}

#[test]
fn withdraw_works() {
	ExtBuilder::default().module_balance(1000).build().execute_with(|| {
		<Balances<Runtime>>::insert(ALICE, 1000);
		assert_ok!(MarginProtocol::withdraw(Origin::signed(ALICE), 500));

		let event = TestEvent::margin_protocol(RawEvent::Withdrew(ALICE, 500));
		assert!(System::events().iter().any(|record| record.event == event));
	});
}

#[test]
fn withdraw_fails_if_balance_too_low() {
	ExtBuilder::default().module_balance(1000).build().execute_with(|| {
		<Balances<Runtime>>::insert(ALICE, 1000);
		assert_noop!(
			MarginProtocol::withdraw(Origin::signed(ALICE), 1500),
			Error::<Runtime>::BalanceTooLow
		);
	});
}

#[test]
fn withdraw_fails_if_trader_would_not_be_safe() {
	ExtBuilder::default()
		.spread(Permill::zero())
		.accumulated_swap_rate(EUR_USD_PAIR, Fixed128::from_natural(1))
		.price(CurrencyId::FEUR, (1, 1))
		.trader_risk_threshold(risk_threshold(100, 0))
		.build()
		.execute_with(|| {
			<Balances<Runtime>>::insert(ALICE, balance_from_natural_currency_cent(100));
			let position: Position<Runtime> = Position {
				owner: ALICE,
				pool: MOCK_POOL,
				pair: EUR_USD_PAIR,
				leverage: Leverage::LongTwo,
				leveraged_held: fixed128_from_natural_currency_cent(100),
				leveraged_debits: fixed128_from_natural_currency_cent(100),
				leveraged_debits_in_usd: fixed128_from_natural_currency_cent(100),
				open_accumulated_swap_rate: Fixed128::from_natural(1),
				open_margin: balance_from_natural_currency_cent(100),
			};
			<Positions<Runtime>>::insert(0, position);
			<PositionsByTrader<Runtime>>::insert(ALICE, MOCK_POOL, vec![0]);
			assert_eq!(
				MarginProtocol::_margin_level(&ALICE, None, None),
				Ok(Fixed128::from_natural(1))
			);

			assert_noop!(
				MarginProtocol::withdraw(Origin::signed(ALICE), 10),
				Error::<Runtime>::TraderWouldBeUnsafe
			);
		});
}

#[test]
fn offchain_worker_should_work() {
	let mut ext = ExtBuilder::default()
		.spread(Permill::zero())
		.accumulated_swap_rate(EUR_USD_PAIR, Fixed128::from_natural(1))
		.price(CurrencyId::FEUR, (1, 1))
		.pool_liquidity(MOCK_POOL, balance_from_natural_currency_cent(100))
		.liquidity_pool_ell_threshold(risk_threshold(100, 0))
		.liquidity_pool_enp_threshold(risk_threshold(100, 0))
		.build();

	let (offchain, _state) = TestOffchainExt::new();
	let (pool, _state) = TestTransactionPoolExt::new();
	ext.register_extension(OffchainExt::new(offchain));
	ext.register_extension(TransactionPoolExt::new(pool));

	ext.execute_with(|| {
		let position: Position<Runtime> = Position {
			owner: ALICE,
			pool: MOCK_POOL,
			pair: EUR_USD_PAIR,
			leverage: Leverage::LongTwo,
			leveraged_held: fixed128_from_natural_currency_cent(100),
			leveraged_debits: fixed128_from_natural_currency_cent(100),
			leveraged_debits_in_usd: fixed128_from_natural_currency_cent(100),
			open_accumulated_swap_rate: Fixed128::from_natural(1),
			open_margin: balance_from_natural_currency_cent(100),
		};

		<Positions<Runtime>>::insert(0, position);
		PositionsByPool::insert(MOCK_POOL, EUR_USD_PAIR, vec![0]);
		<PositionsByTrader<Runtime>>::insert(ALICE, MOCK_POOL, vec![0]);

		assert_noop!(
			MarginProtocol::_ensure_trader_safe(&ALICE, None, None),
			Error::<Runtime>::UnsafeTrader
		);
		assert_noop!(
			MarginProtocol::_ensure_pool_safe(MOCK_POOL, None, None),
			Error::<Runtime>::UnsafePool
		);

		assert_eq!(MarginProtocol::_check_all_traders(), Ok((vec![ALICE], vec![], vec![])));
		assert_eq!(
			MarginProtocol::_check_all_pools(),
			Ok((vec![], vec![MOCK_POOL], vec![]))
		);

		assert_ok!(MarginProtocol::_offchain_worker(1));
	});
}

#[test]
fn liquidity_pool_manager_can_remove_works() {
	ExtBuilder::default().build().execute_with(|| {
		assert!(<MarginProtocol as LiquidityPoolManager<LiquidityPoolId, Balance>>::can_remove(MOCK_POOL));

		<Positions<Runtime>>::insert(0, eur_jpy_long());
		PositionsByPool::insert(MOCK_POOL, EUR_JPY_PAIR, vec![0]);
		assert!(!<MarginProtocol as LiquidityPoolManager<LiquidityPoolId, Balance>>::can_remove(MOCK_POOL));
	});
}

#[test]
fn liquidity_pool_manager_get_required_deposit_works() {
	ExtBuilder::default()
		.spread(Permill::zero())
		.accumulated_swap_rate(EUR_USD_PAIR, Fixed128::from_natural(1))
		.price(CurrencyId::FEUR, (1, 1))
		.pool_liquidity(MOCK_POOL, balance_from_natural_currency_cent(0))
		.liquidity_pool_ell_threshold(risk_threshold(90, 0))
		.liquidity_pool_enp_threshold(risk_threshold(100, 0))
		.build()
		.execute_with(|| {
			let position: Position<Runtime> = Position {
				owner: ALICE,
				pool: MOCK_POOL,
				pair: EUR_USD_PAIR,
				leverage: Leverage::LongTwo,
				leveraged_held: fixed128_from_natural_currency_cent(100),
				leveraged_debits: fixed128_from_natural_currency_cent(100),
				leveraged_debits_in_usd: fixed128_from_natural_currency_cent(100),
				open_accumulated_swap_rate: Fixed128::from_natural(1),
				open_margin: balance_from_natural_currency_cent(100),
			};
			let id = 0;
			<Positions<Runtime>>::insert(id, position);
			PositionsByPool::insert(MOCK_POOL, EUR_USD_PAIR, vec![id]);

			// need deposit because of ENP
			assert_eq!(
				MarginProtocol::get_required_deposit(MOCK_POOL),
				Ok(balance_from_natural_currency_cent(100)),
			);

			// need deposit because of ELL
			LiquidityPoolENPThreshold::put(risk_threshold(80, 0));
			assert_eq!(
				MarginProtocol::get_required_deposit(MOCK_POOL),
				Ok(balance_from_natural_currency_cent(90)),
			);

			// no need to deposit
			MockLiquidityPools::set_mock_liquidity(MOCK_POOL, balance_from_natural_currency_cent(100));
			assert_eq!(
				MarginProtocol::get_required_deposit(MOCK_POOL),
				Ok(balance_from_natural_currency_cent(0)),
			);
		});
}
