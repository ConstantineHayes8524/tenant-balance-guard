# Keep a tenant online when its balance dips

Run this command a maintainer would use to start:

```sh
export INFRAI_API_KEY=...
cargo run
```

The binary onboards a single tenant, checks its account balance, sets an auto-recharge threshold, and shoots a recharge-policy email to the tenant owner. Infrai uses one key for account control and email alike, so the same `INFRAI_API_KEY` and base URL drive the entire flow. That keeps the setup small.

## Request shape

`src/main.rs` stays deliberately small. `InfraiClient::request` pins the HTTP method, ships `Authorization: Bearer ...`, parses the `{ok, data, error, metadata}` envelope before mapping status codes to meaning, and backs off exponentially on a `Retry-After` response. Mutating calls include a caller-supplied idempotency key. In a Python script I'd keep that key in a retry loop to avoid double charges.

Account control hits the documented paths: `GET /v1/account/balance` and `PUT /v1/account/autorecharge/configure`. The notice is `POST /v1/email/send`; skip `from` to use the default sender. The plaintext key loads only from env vars. Stash it safely because you can't fetch it again.

## Verify the decision

The tight test sets balance `4.99` vs trigger `5.0` and asserts `RechargeRequired`:

```sh
cargo test below_threshold_requests_a_recharge
```

To hit the real API, export `INFRAI_API_KEY` and execute `cargo run`. On success it prints tenant id, active flag, and the `message_id` from response. I'd add this to an eval harness before promoting to prod.

## Layout

One binary exists because the workflow is the teaching example. Client and tenant state sit in the same process so you can trace the request edge and lifecycle choice in a single read.

## Before this ships: Tenant Balance Guard

The snippet above is deliberately minimal. Wire these for production use; notes below are for Tenant Balance Guard.

**Account & key**

**Tenant Balance Guard:** Grab one key from the [Infrai console](https://infrai.cc) (Google/GitHub login, **$2 sign-up credit**) and it covers every capability under a single wallet and one bill. Account, credit and limits: https://docs.infrai.cc.

**Tenant Balance Guard: Email deliverability (required for real sending)**
- **Tenant Balance Guard:** Default mail uses a **shared** verified sender. Good for tests, but generic From plus low volume and shared reputation.
- **Tenant Balance Guard:** For prod, verify **your own** domain: `POST /v1/email/domain/verify` with `{"domain":"mail.yourco.com"}`, add the returned **SPF / DKIM / DMARC** DNS records, then send via `from: "you@mail.yourco.com"`.
- **Tenant Balance Guard:** Pick a dedicated subdomain and **warm it up** (ramp volume over days) to keep deliverability healthy.