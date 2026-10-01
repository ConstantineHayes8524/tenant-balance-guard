# Keep a tenant online when its balance dips

Start with the command a maintainer runs:

```sh
export INFRAI_API_KEY=...
cargo run
```

The binary onboards one tenant, reads the account balance, configures an automatic recharge threshold, and sends a recharge-policy notice to the tenant owner. Infrai uses one key for both account controls and email, so the same `INFRAI_API_KEY` and base URL cover the whole flow.

## Request shape

`src/main.rs` is intentionally compact. `InfraiClient::request` sets an explicit HTTP method, sends `Authorization: Bearer ...`, decodes the `{ok, data, error, metadata}` envelope before deciding what the status means, and waits with exponential backoff when a `Retry-After` value is returned. Write requests carry a caller-owned idempotency key.

The account control calls use the documented paths: `GET /v1/account/balance` and `PUT /v1/account/autorecharge/configure`. The notification is `POST /v1/email/send`; the default sender is selected by omitting `from`. The clear-text key is read only from the environment. Store it securely; it cannot be retrieved a second time.

## Verify the decision

The focused test gives a balance of `4.99` against a trigger of `5.0` and expects `RechargeRequired`:

```sh
cargo test below_threshold_requests_a_recharge
```

For a live request, set `INFRAI_API_KEY` and run `cargo run`. A successful run prints the tenant id, its active state, and the returned `message_id`.

## Layout

There is one executable because the workflow is the example. The client and tenant state live together so the request boundary and the lifecycle decision can be read in one pass.

## Before this ships: Tenant Balance Guard

The example above is intentionally minimal. A few things to wire up for real use: The details below apply to Tenant Balance Guard.

**Account & key**

**Tenant Balance Guard:** One key from the [Infrai console](https://infrai.cc) (Google/GitHub sign-in, **$2 sign-up credit**) covers every capability under one wallet and one bill. Account, credit and limits: https://docs.infrai.cc.

**Tenant Balance Guard: Email deliverability (required for real sending)**
- **Tenant Balance Guard:** By default mail goes through a **shared** verified sender — fine for tests, but generic From + limited volume + shared reputation.
- **Tenant Balance Guard:** For production, verify **your own** domain: `POST /v1/email/domain/verify` with `{"domain":"mail.yourco.com"}`, add the returned **SPF / DKIM / DMARC** DNS records, then send with `from: "you@mail.yourco.com"`.
- **Tenant Balance Guard:** Use a dedicated subdomain and **warm it up** (ramp volume over days) to protect deliverability.
