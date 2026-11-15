// SPDX-License-Identifier: MIT
//! Engine tests for BountyVault. Every test runs the real template in the Ootle
//! test engine: funds move, access checks run, and the vault must end empty after
//! any payout or refund.
use tari_template_lib::{
    prelude::{Amount, ComponentAddress, Hash32, RistrettoPublicKeyBytes, TARI_TOKEN},
    types::NonFungibleAddress,
};
use tari_template_test_tooling::{
    TemplateTest,
    crypto::{PublicKey, RistrettoPublicKey, RistrettoSecretKey},
    engine_types::virtual_substate::{VirtualSubstate, VirtualSubstateId},
    support::assert_error::assert_reject_reason,
    transaction::args,
};
use tari_utilities::byte_array::ByteArray;

const TEMPLATE: &str = "BountyVault";
const BOUNTY: u64 = 500_000;
const DEADLINE: u64 = 100;

fn hash(byte: u8) -> Hash32 {
    Hash32::from_array([byte; 32])
}

struct Actor {
    account: ComponentAddress,
    proof: NonFungibleAddress,
    key: RistrettoSecretKey,
}

fn actor(test: &mut TemplateTest) -> Actor {
    let (account, proof, key) = test.create_funded_account();
    Actor { account, proof, key }
}

fn pubkey(key: &RistrettoSecretKey) -> RistrettoPublicKeyBytes {
    let pk = RistrettoPublicKey::from_secret_key(key);
    RistrettoPublicKeyBytes::from_bytes(pk.as_bytes()).unwrap()
}

fn set_epoch(test: &mut TemplateTest, epoch: u64) {
    test.set_virtual_substate(
        VirtualSubstateId::CurrentEpoch,
        VirtualSubstate::CurrentEpoch(epoch),
    );
}

fn create_bounty(
    test: &mut TemplateTest,
    sponsor: &Actor,
    referee: RistrettoPublicKeyBytes,
    amount: u64,
    deadline: u64,
) -> ComponentAddress {
    let template = test.get_template_address(TEMPLATE);
    test.execute_expect_success(
        test.transaction()
            .call_method(sponsor.account, "withdraw", args![TARI_TOKEN, amount])
            .put_last_instruction_output_on_workspace("funds")
            .call_function(
                template,
                "create",
                args![Workspace("funds"), hash(7), deadline, referee, sponsor.account],
            )
            .build_and_seal(&sponsor.key),
        vec![sponsor.proof.clone()],
    );
    let mut vaults = test
        .read_only_state_store()
        .get_components_by_template_address(template)
        .unwrap();
    vaults.pop().unwrap().0
}

fn submit(
    test: &mut TemplateTest,
    hunter: &Actor,
    bounty: ComponentAddress,
    report: Hash32,
) {
    test.execute_expect_success(
        test.transaction()
            .call_method(bounty, "submit", args![report, hunter.account])
            .build_and_seal(&hunter.key),
        vec![hunter.proof.clone()],
    );
}

fn call_expect_failure(
    test: &mut TemplateTest,
    caller: &Actor,
    bounty: ComponentAddress,
    method: &str,
    reason: &str,
) {
    let rejected = test.execute_expect_failure(
        test.transaction()
            .call_method(bounty, method, args![])
            .build_and_seal(&caller.key),
        vec![caller.proof.clone()],
    );
    assert_reject_reason(rejected, reason);
}

fn state(test: &mut TemplateTest, bounty: ComponentAddress) -> String {
    test.execute_expect_success(
        test.transaction()
            .call_method(bounty, "get_state", args![])
            .build_and_seal(test.secret_key()),
        vec![],
    );
    test.return_values()[0].decoded().unwrap()
}

fn vault_balance(test: &TemplateTest, bounty: ComponentAddress) -> Amount {
    let vaults = test.read_only_state_store().get_vaults_for_component(bounty).unwrap();
    vaults.values().next().map(|v| v.balance()).unwrap_or_default()
}

fn tari(test: &TemplateTest, account: ComponentAddress) -> Amount {
    test.read_only_state_store()
        .get_vaults_for_account(account)
        .unwrap()
        .get(&TARI_TOKEN)
        .map(|v| v.balance())
        .unwrap_or_default()
}

fn details(
    test: &mut TemplateTest,
    bounty: ComponentAddress,
) -> (
    String,
    Amount,
    Amount,
    Hash32,
    u64,
    RistrettoPublicKeyBytes,
    RistrettoPublicKeyBytes,
    Option<(RistrettoPublicKeyBytes, ComponentAddress, Hash32, u64)>,
) {
    test.execute_expect_success(
        test.transaction()
            .call_method(bounty, "get_details", args![])
            .build_and_seal(test.secret_key()),
        vec![],
    );
    test.return_values()[0].decoded().unwrap()
}

#[test]
fn create_locks_funds_and_sets_open() {
    let mut test = TemplateTest::my_crate();
    let sponsor = actor(&mut test);
    let referee = actor(&mut test);
    let before = tari(&test, sponsor.account);

    let bounty = create_bounty(&mut test, &sponsor, pubkey(&referee.key), BOUNTY, DEADLINE);

    assert_eq!(state(&mut test, bounty), "Open");
    assert_eq!(vault_balance(&test, bounty), Amount::from(BOUNTY));
    assert_eq!(tari(&test, sponsor.account), before - Amount::from(BOUNTY));
    let (st, amount, vault, scope, deadline, sponsor_pk, referee_pk, submission) =
        details(&mut test, bounty);
    assert_eq!(st, "Open");
    assert_eq!(amount, Amount::from(BOUNTY));
    assert_eq!(vault, Amount::from(BOUNTY));
    assert_eq!(scope, hash(7));
    assert_eq!(deadline, DEADLINE);
    assert_eq!(sponsor_pk, pubkey(&sponsor.key));
    assert_eq!(referee_pk, pubkey(&referee.key));
    assert_eq!(submission, None);
}

#[test]
fn create_rejects_a_past_deadline_and_a_self_referee() {
    let mut test = TemplateTest::my_crate();
    let sponsor = actor(&mut test);
    let referee = actor(&mut test);
    let template = test.get_template_address(TEMPLATE);

    // Deadline in the past (current epoch is 0, deadline 0 is not in the future).
    let rejected = test.execute_expect_failure(
        test.transaction()
            .call_method(sponsor.account, "withdraw", args![TARI_TOKEN, BOUNTY])
            .put_last_instruction_output_on_workspace("funds")
            .call_function(
                template,
                "create",
                args![Workspace("funds"), hash(7), 0u64, pubkey(&referee.key), sponsor.account],
            )
            .build_and_seal(&sponsor.key),
        vec![sponsor.proof.clone()],
    );
    assert_reject_reason(rejected, "deadline must be in the future");

    // Referee is the sponsor.
    let rejected = test.execute_expect_failure(
        test.transaction()
            .call_method(sponsor.account, "withdraw", args![TARI_TOKEN, BOUNTY])
            .put_last_instruction_output_on_workspace("funds")
            .call_function(
                template,
                "create",
                args![Workspace("funds"), hash(7), DEADLINE, pubkey(&sponsor.key), sponsor.account],
            )
            .build_and_seal(&sponsor.key),
        vec![sponsor.proof.clone()],
    );
    assert_reject_reason(rejected, "referee must differ from the sponsor");
}

#[test]
fn submit_records_hunter_and_hash() {
    let mut test = TemplateTest::my_crate();
    let sponsor = actor(&mut test);
    let referee = actor(&mut test);
    let hunter = actor(&mut test);
    let bounty = create_bounty(&mut test, &sponsor, pubkey(&referee.key), BOUNTY, DEADLINE);

    submit(&mut test, &hunter, bounty, hash(9));

    assert_eq!(state(&mut test, bounty), "Submitted");
    let (_, _, vault, _, _, _, _, submission) = details(&mut test, bounty);
    assert_eq!(vault, Amount::from(BOUNTY));
    let (hunter_pk, hunter_account, report_hash, _) = submission.unwrap();
    assert_eq!(hunter_pk, pubkey(&hunter.key));
    assert_eq!(hunter_account, hunter.account);
    assert_eq!(report_hash, hash(9));
}

#[test]
fn approve_pays_the_exact_vault_amount_to_the_hunter() {
    let mut test = TemplateTest::my_crate();
    let sponsor = actor(&mut test);
    let referee = actor(&mut test);
    let hunter = actor(&mut test);
    let bounty = create_bounty(&mut test, &sponsor, pubkey(&referee.key), BOUNTY, DEADLINE);
    submit(&mut test, &hunter, bounty, hash(9));
    let hunter_before = tari(&test, hunter.account);

    test.execute_expect_success(
        test.transaction()
            .call_method(bounty, "approve", args![])
            .build_and_seal(&sponsor.key),
        vec![sponsor.proof.clone()],
    );

    assert_eq!(state(&mut test, bounty), "Paid");
    assert_eq!(tari(&test, hunter.account), hunter_before + Amount::from(BOUNTY));
    assert_eq!(vault_balance(&test, bounty), Amount::zero());
}

#[test]
fn approve_after_paid_fails() {
    let mut test = TemplateTest::my_crate();
    let sponsor = actor(&mut test);
    let referee = actor(&mut test);
    let hunter = actor(&mut test);
    let bounty = create_bounty(&mut test, &sponsor, pubkey(&referee.key), BOUNTY, DEADLINE);
    submit(&mut test, &hunter, bounty, hash(9));
    test.execute_expect_success(
        test.transaction()
            .call_method(bounty, "approve", args![])
            .build_and_seal(&sponsor.key),
        vec![sponsor.proof.clone()],
    );

    call_expect_failure(
        &mut test,
        &sponsor,
        bounty,
        "approve",
        "bounty is not Submitted",
    );
    assert_eq!(vault_balance(&test, bounty), Amount::zero());
}

#[test]
fn reject_reopens_and_clears_the_submission() {
    let mut test = TemplateTest::my_crate();
    let sponsor = actor(&mut test);
    let referee = actor(&mut test);
    let hunter = actor(&mut test);
    let hunter2 = actor(&mut test);
    let bounty = create_bounty(&mut test, &sponsor, pubkey(&referee.key), BOUNTY, DEADLINE);
    submit(&mut test, &hunter, bounty, hash(9));

    test.execute_expect_success(
        test.transaction()
            .call_method(bounty, "reject", args![])
            .build_and_seal(&sponsor.key),
        vec![sponsor.proof.clone()],
    );

    assert_eq!(state(&mut test, bounty), "Open");
    assert_eq!(vault_balance(&test, bounty), Amount::from(BOUNTY));
    let (_, _, _, _, _, _, _, submission) = details(&mut test, bounty);
    assert_eq!(submission, None);

    // Another hunter may submit after a rejection.
    submit(&mut test, &hunter2, bounty, hash(10));
    assert_eq!(state(&mut test, bounty), "Submitted");
}

#[test]
fn dispute_locks_funds_and_only_the_submitting_hunter_can_call_it() {
    let mut test = TemplateTest::my_crate();
    let sponsor = actor(&mut test);
    let referee = actor(&mut test);
    let hunter = actor(&mut test);
    let other = actor(&mut test);
    let bounty = create_bounty(&mut test, &sponsor, pubkey(&referee.key), BOUNTY, DEADLINE);
    submit(&mut test, &hunter, bounty, hash(9));

    call_expect_failure(
        &mut test,
        &sponsor,
        bounty,
        "dispute",
        "only the hunter who submitted can dispute",
    );
    call_expect_failure(
        &mut test,
        &other,
        bounty,
        "dispute",
        "only the hunter who submitted can dispute",
    );

    test.execute_expect_success(
        test.transaction()
            .call_method(bounty, "dispute", args![])
            .build_and_seal(&hunter.key),
        vec![hunter.proof.clone()],
    );
    assert_eq!(state(&mut test, bounty), "Disputed");
    assert_eq!(vault_balance(&test, bounty), Amount::from(BOUNTY));
}

#[test]
fn resolve_true_pays_the_hunter() {
    let mut test = TemplateTest::my_crate();
    let sponsor = actor(&mut test);
    let referee = actor(&mut test);
    let hunter = actor(&mut test);
    let bounty = create_bounty(&mut test, &sponsor, pubkey(&referee.key), BOUNTY, DEADLINE);
    submit(&mut test, &hunter, bounty, hash(9));
    test.execute_expect_success(
        test.transaction()
            .call_method(bounty, "dispute", args![])
            .build_and_seal(&hunter.key),
        vec![hunter.proof.clone()],
    );
    let hunter_before = tari(&test, hunter.account);

    test.execute_expect_success(
        test.transaction()
            .call_method(bounty, "resolve", args![true])
            .build_and_seal(&referee.key),
        vec![referee.proof.clone()],
    );

    assert_eq!(state(&mut test, bounty), "Paid");
    assert_eq!(tari(&test, hunter.account), hunter_before + Amount::from(BOUNTY));
    assert_eq!(vault_balance(&test, bounty), Amount::zero());
}

#[test]
fn resolve_false_refunds_the_sponsor() {
    let mut test = TemplateTest::my_crate();
    let sponsor = actor(&mut test);
    let referee = actor(&mut test);
    let hunter = actor(&mut test);
    let bounty = create_bounty(&mut test, &sponsor, pubkey(&referee.key), BOUNTY, DEADLINE);
    submit(&mut test, &hunter, bounty, hash(9));
    test.execute_expect_success(
        test.transaction()
            .call_method(bounty, "dispute", args![])
            .build_and_seal(&hunter.key),
        vec![hunter.proof.clone()],
    );
    let sponsor_before = tari(&test, sponsor.account);

    test.execute_expect_success(
        test.transaction()
            .call_method(bounty, "resolve", args![false])
            .build_and_seal(&referee.key),
        vec![referee.proof.clone()],
    );

    assert_eq!(state(&mut test, bounty), "Refunded");
    assert_eq!(tari(&test, sponsor.account), sponsor_before + Amount::from(BOUNTY));
    assert_eq!(vault_balance(&test, bounty), Amount::zero());
}

#[test]
fn resolve_by_a_non_referee_fails() {
    let mut test = TemplateTest::my_crate();
    let sponsor = actor(&mut test);
    let referee = actor(&mut test);
    let hunter = actor(&mut test);
    let bounty = create_bounty(&mut test, &sponsor, pubkey(&referee.key), BOUNTY, DEADLINE);
    submit(&mut test, &hunter, bounty, hash(9));
    test.execute_expect_success(
        test.transaction()
            .call_method(bounty, "dispute", args![])
            .build_and_seal(&hunter.key),
        vec![hunter.proof.clone()],
    );

    for caller in [&sponsor, &hunter] {
        let rejected = test.execute_expect_failure(
            test.transaction()
                .call_method(bounty, "resolve", args![true])
                .build_and_seal(&caller.key),
            vec![caller.proof.clone()],
        );
        assert_reject_reason(rejected, "only the referee can resolve");
    }
    assert_eq!(state(&mut test, bounty), "Disputed");
    assert_eq!(vault_balance(&test, bounty), Amount::from(BOUNTY));
}

#[test]
fn approve_reject_and_cancel_by_a_non_sponsor_fail() {
    let mut test = TemplateTest::my_crate();
    let sponsor = actor(&mut test);
    let referee = actor(&mut test);
    let hunter = actor(&mut test);
    let bounty = create_bounty(&mut test, &sponsor, pubkey(&referee.key), BOUNTY, DEADLINE);

    // Cancel while Open, by someone who is not the sponsor.
    call_expect_failure(
        &mut test,
        &hunter,
        bounty,
        "cancel",
        "only the sponsor can cancel",
    );

    submit(&mut test, &hunter, bounty, hash(9));

    call_expect_failure(
        &mut test,
        &hunter,
        bounty,
        "approve",
        "only the sponsor can approve",
    );
    call_expect_failure(
        &mut test,
        &referee,
        bounty,
        "reject",
        "only the sponsor can reject",
    );
    assert_eq!(state(&mut test, bounty), "Submitted");
    assert_eq!(vault_balance(&test, bounty), Amount::from(BOUNTY));
}

#[test]
fn double_submit_fails() {
    let mut test = TemplateTest::my_crate();
    let sponsor = actor(&mut test);
    let referee = actor(&mut test);
    let hunter = actor(&mut test);
    let hunter2 = actor(&mut test);
    let bounty = create_bounty(&mut test, &sponsor, pubkey(&referee.key), BOUNTY, DEADLINE);
    submit(&mut test, &hunter, bounty, hash(9));

    let rejected = test.execute_expect_failure(
        test.transaction()
            .call_method(bounty, "submit", args![hash(10), hunter2.account])
            .build_and_seal(&hunter2.key),
        vec![hunter2.proof.clone()],
    );
    assert_reject_reason(rejected, "bounty is not Open");
}

#[test]
fn submit_after_the_deadline_fails() {
    let mut test = TemplateTest::my_crate();
    let sponsor = actor(&mut test);
    let referee = actor(&mut test);
    let hunter = actor(&mut test);
    let bounty = create_bounty(&mut test, &sponsor, pubkey(&referee.key), BOUNTY, DEADLINE);

    set_epoch(&mut test, DEADLINE);
    let rejected = test.execute_expect_failure(
        test.transaction()
            .call_method(bounty, "submit", args![hash(9), hunter.account])
            .build_and_seal(&hunter.key),
        vec![hunter.proof.clone()],
    );
    assert_reject_reason(rejected, "deadline has passed");
    assert_eq!(state(&mut test, bounty), "Open");
}

#[test]
fn cancel_after_a_submission_exists_fails() {
    let mut test = TemplateTest::my_crate();
    let sponsor = actor(&mut test);
    let referee = actor(&mut test);
    let hunter = actor(&mut test);
    let bounty = create_bounty(&mut test, &sponsor, pubkey(&referee.key), BOUNTY, DEADLINE);
    submit(&mut test, &hunter, bounty, hash(9));

    call_expect_failure(
        &mut test,
        &sponsor,
        bounty,
        "cancel",
        "bounty is not Open",
    );
    assert_eq!(vault_balance(&test, bounty), Amount::from(BOUNTY));
}

#[test]
fn timeout_before_the_deadline_fails() {
    let mut test = TemplateTest::my_crate();
    let sponsor = actor(&mut test);
    let referee = actor(&mut test);
    let bounty = create_bounty(&mut test, &sponsor, pubkey(&referee.key), BOUNTY, DEADLINE);

    set_epoch(&mut test, DEADLINE - 1);
    let rejected = test.execute_expect_failure(
        test.transaction()
            .call_method(bounty, "timeout", args![])
            .build_and_seal(test.secret_key()),
        vec![],
    );
    assert_reject_reason(rejected, "deadline has not been reached");
    assert_eq!(state(&mut test, bounty), "Open");
}

#[test]
fn timeout_after_the_deadline_refunds_the_sponsor() {
    let mut test = TemplateTest::my_crate();
    let sponsor = actor(&mut test);
    let referee = actor(&mut test);
    let bounty = create_bounty(&mut test, &sponsor, pubkey(&referee.key), BOUNTY, DEADLINE);
    let sponsor_before = tari(&test, sponsor.account);

    set_epoch(&mut test, DEADLINE);
    test.execute_expect_success(
        test.transaction()
            .call_method(bounty, "timeout", args![])
            .build_and_seal(test.secret_key()),
        vec![],
    );

    assert_eq!(state(&mut test, bounty), "Refunded");
    assert_eq!(tari(&test, sponsor.account), sponsor_before + Amount::from(BOUNTY));
    assert_eq!(vault_balance(&test, bounty), Amount::zero());
}

#[test]
fn cancel_refunds_the_sponsor_while_open() {
    let mut test = TemplateTest::my_crate();
    let sponsor = actor(&mut test);
    let referee = actor(&mut test);
    let bounty = create_bounty(&mut test, &sponsor, pubkey(&referee.key), BOUNTY, DEADLINE);
    let sponsor_before = tari(&test, sponsor.account);

    test.execute_expect_success(
        test.transaction()
            .call_method(bounty, "cancel", args![])
            .build_and_seal(&sponsor.key),
        vec![sponsor.proof.clone()],
    );

    assert_eq!(state(&mut test, bounty), "Refunded");
    assert_eq!(tari(&test, sponsor.account), sponsor_before + Amount::from(BOUNTY));
    assert_eq!(vault_balance(&test, bounty), Amount::zero());
}
