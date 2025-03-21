pds-migrate
===========
Rust CLI tool for migrating an ATProto account to a new PDS. Based on the example TS code provided at https://github.com/bluesky-social/pds/blob/main/ACCOUNT_MIGRATION.md ported to Rust using [ATrium](https://github.com/sugyan/atrium)

The current implementation is quite basic and is not guarenteed not to brick your account. While it should work just fine it has not been thoroughly tested and does not yet do any verification during the migration nor does it handle any potential previous partially finished migrations.
