use cosmwasm_std::Uint128;
use cw_controllers::Claim;

use cw20_vesting::ContractError as VestingContractError;
use wynd_utils::Curve;

use super::suite::{SuiteBuilder, SEVEN_DAYS};

const START: u64 = 1671797419; // way after env's timestamp at start to keep tokens vested
const END: u64 = START + 10_000;

#[test]
fn delegate_and_unbond_tokens_still_vested() {
    let user = "user";
    let mut suite = SuiteBuilder::new()
        .with_initial_balances(vec![(
            user,
            100_000,
            Curve::saturating_linear((START, 100_000), (END, 0)),
        )])
        .build();

    assert_eq!(
        suite.query_balance_vesting_contract(user).unwrap(),
        100_000u128
    );

    // delegate half of the tokens, ensure they are staked
    suite.delegate(user, 50_000u128, None).unwrap();
    assert_eq!(suite.query_staked(user, None).unwrap(), 50_000u128);
    assert_eq!(
        suite.query_balance_vesting_contract(user).unwrap(),
        50_000u128
    );
    assert_eq!(
        suite
            .query_balance_vesting_contract(&suite.stake_contract())
            .unwrap(),
        50_000u128
    );

    // undelegate and unbond all
    suite.unbond(user, 50_000u128, None).unwrap();
    // nothing is staked
    assert_eq!(suite.query_staked(user, None).unwrap(), 0u128);
    // Balance is the same until claim is available
    assert_eq!(
        suite.query_balance_vesting_contract(user).unwrap(),
        50_000u128
    );

    let claims = suite.query_claims(user).unwrap();
    assert_eq!(claims.len(), 1);
    assert!(matches!(
        claims[0],
        Claim {
            amount,
            ..
        } if amount == Uint128::new(50_000)
    ));

    suite.update_time(SEVEN_DAYS * 2); // update height to simulate passing time
    suite.claim(user).unwrap();
    let claims = suite.query_claims(user).unwrap();
    assert_eq!(claims.len(), 0);
    // after expiration time passed, tokens can be claimed and transferred back to user account
    assert_eq!(
        suite.query_balance_vesting_contract(user).unwrap(),
        100_000u128
    );

    // user still cannot transfer any token, because it's all vested
    let err = suite.transfer(user, "random_user", 1u128).unwrap_err();
    assert_eq!(
        VestingContractError::CantMoveVestingTokens,
        err.downcast().unwrap()
    );
}

#[test]
fn mixed_vested_liquid_delegate_and_transfer_remaining() {
    let user = "user";
    let mut suite = SuiteBuilder::new()
        .with_initial_balances(vec![(
            user,
            100_000,
            // half of amount is vested
            Curve::saturating_linear((START, 50_000), (END, 0)),
        )])
        .build();

    assert_eq!(
        suite.query_balance_vesting_contract(user).unwrap(),
        100_000u128
    );

    suite.delegate(user, 60_000u128, None).unwrap(); // delegate some of vested tokens as well
    assert_eq!(suite.query_staked(user, None).unwrap(), 60_000u128);
    assert_eq!(
        suite.query_balance_vesting_contract(user).unwrap(),
        40_000u128
    );

    // transfer remaining 40_000 to a different address, to show that vested tokens are delegated
    // first
    assert_eq!(
        suite.query_balance_vesting_contract(user).unwrap(),
        40_000u128
    );
    suite.transfer(user, "random_user", 40_000u128).unwrap();

    assert_eq!(suite.query_balance_vesting_contract(user).unwrap(), 0u128); // user has empty
                                                                            // account now

    // undelegate some of the tokens
    suite.unbond(user, 20_000u128, None).unwrap();
    assert_eq!(suite.query_staked(user, None).unwrap(), 40_000u128); // 60_000 delegated - 20_000
                                                                     // unbonded
    assert_eq!(suite.query_balance_vesting_contract(user).unwrap(), 0u128);

    let claims = suite.query_claims(user).unwrap();
    assert!(matches!(
        claims[0],
        Claim {
            amount,
            ..
        } if amount == Uint128::new(20_000)
    ));

    suite.update_time(SEVEN_DAYS); // update height to simulate passing time
    suite.claim(user).unwrap();
    assert_eq!(
        suite.query_balance_vesting_contract(user).unwrap(),
        20_000u128
    );

    // now user received 20_000 unbonded tokens
    // 10_000 should be allowed to transfer, while the rest should be locked because of vesting
    suite.transfer(user, "random_user", 10_000u128).unwrap(); // works
    let err = suite.transfer(user, "random_user", 1u128).unwrap_err(); // this is vested amount
    assert_eq!(
        VestingContractError::CantMoveVestingTokens,
        err.downcast().unwrap()
    );
}
