# Contributing

## Licensing of contributions

TinyLVT's crates carry different licenses (see [LICENSE.md](LICENSE.md)), but all contributions to this repository, regardless of which crate they touch, are accepted under the MIT license. By submitting a contribution (a pull request, patch, or similar), you agree that your contribution is licensed under MIT (see [LICENSE-MIT](LICENSE-MIT)) and you confirm that you have the right to submit it under those terms.

Why MIT inbound: it lets the project move code freely between the AGPL and MIT crates, for example promoting a validation helper from `api` into `payloads` so clients can run the same checks before sending requests, without tracking down every past author for permission. MIT-licensed contributions are fully compatible with distribution inside the AGPL crates. Note that this means your contribution carries the most permissive terms used anywhere in the project.

Copyright holders are never bound by the licenses they grant, so the maintainer's own code in `api` is movable to `payloads` by author's right even though it is distributed as AGPL. The inbound-MIT policy puts external contributions on the same footing, so that any line in the repository can cross the AGPL/MIT boundary when it becomes useful to clients, without a per-author permissions hunt.

Note that this policy is not a CLA. Your contribution is licensed under MIT to everyone, not granted specially to the maintainer: every fork of this repository holds exactly the same rights over your contribution that the upstream project does. The inbound grant exists so that code can move toward more permissive terms, never to reduce what is openly available. The `api` and `ui` crates will continue to be distributed under the AGPL.
