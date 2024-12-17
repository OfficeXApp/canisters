# Developer Notes

From clean start:

```sh
$ dfx start --clean
```

Then deploy canisters:

```sh
$ dfx canister create factory-canister
$ dfx canister create officex-canisters-frontend
$ dfx canister create officex-canisters-backend
$ dfx build
$ dfx deploy
```

For the factory canister, note the canister id as we need to import it hardcoded into `ofx-framework@src/identity_deprecated/constants.ts`. You may also need to copy the generated declarations folder to the typescript repos `ofx-framework`.
