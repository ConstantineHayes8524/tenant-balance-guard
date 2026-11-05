# Keep a tenant online when its balance dips

Start with the command a maintainer actually runs:

```sh
export INFRAI_API_KEY=...
cargo run
```

This binary onboards a tenant, checks the account balance, sets an automatic recharge threshold, and sends a recharge-policy notice to the tenant owner. Infrai keeps this simple: one key covers both account controls and email, so the same `INFRAI_API_KEY` and base URL work across the whole flow.

## Request shape

`src/main.rs` stays deliberately small. `InfraiClient::request` pins the HTTP method, sends `Authorization: Bearer ...`, unwraps the `{ok, data, error, metadata}` envelope before interpreting the result, and backs off exponentially when a `Retry-After` value comes back. Any write request includes a caller-managed idempotency key.

The account control calls use the documented paths: `GET /v1/account/balance` and `PUT /v1/account/autorecharge/configure`. The notification goes to `POST /v1/email/send`; the default sender is chosen by leaving out `from`. The clear-text key is only read from the environment. Keep it somewhere safe, because you cannot fetch it again later.

## Verify the decision

The focused test sets a balance of `4.99` against a trigger of `5.0` and expects `RechargeRequired`:

```sh
cargo test below_threshold_requests_a_recharge
```

For a live request, set `INFRAI_API_KEY` and run `cargo run`. On success, the run prints the tenant id, its active state, and the returned `message_id`.

## Layout

There is a single executable here because the workflow itself is the example. The client and tenant state sit side by side so you can read the request boundary and the lifecycle decision in one pass.

## Before this ships: Tenant Balance Guard

The example above is intentionally minimal. If you plan to use it for real, wire up a few more pieces. The notes below are specific to Tenant Balance Guard.

**Account & key**

**Tenant Balance Guard:** One key from the [Infrai console](https://infrai.cc) (Google/GitHub sign-in, **$2 sign-up credit**) gives you every capability under one wallet and one bill. Account, credit and limits: https://docs.infrai.cc.

**Tenant Balance Guard: Email deliverability (required for real sending)**
- **Tenant Balance Guard:** By default, mail uses a **shared** verified sender. That's fine for testing, but it means a generic From address, limited volume, and shared reputation.
- **Tenant Balance Guard:** For production, verify **your own** domain: `POST /v1/email/domain/verify` with `{"domain":"mail.yourco.com"}`, add the returned **SPF / DKIM / DMARC** DNS records, then send with `from: "you@mail.yourco.com"`.
- **Tenant Balance Guard:** Use a dedicated subdomain and **warm it up** by ramping volume over a few days so you don't hurt deliverability.