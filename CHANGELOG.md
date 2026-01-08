# [1.3.0](https://github.com/bettyblocks/wasm-base-components/compare/v1.2.2...v1.3.0) (2026-01-08)


### Features

* add configurations to payload ([c5548df](https://github.com/bettyblocks/wasm-base-components/commit/c5548dfc47bd92257f82655738646288473d1a0c))

## [1.2.2](https://github.com/bettyblocks/wasm-base-components/compare/v1.2.1...v1.2.2) (2025-12-23)


### Bug Fixes

* rebuild wasm component when changed ([88754de](https://github.com/bettyblocks/wasm-base-components/commit/88754de2b2936db610d9d102ac72aef2ac9827f9))
* release ([857bb7f](https://github.com/bettyblocks/wasm-base-components/commit/857bb7f4e7bbe1498f7e702e2705a5b419e17b07))
* use GH_TOKEN because of protected branches ([2c13a7c](https://github.com/bettyblocks/wasm-base-components/commit/2c13a7cafacbb3f759d9387dba9e18d2492287d8))
* when data-api returns an error propagate the error ([092a85e](https://github.com/bettyblocks/wasm-base-components/commit/092a85e3d6883038dc7fcc56852c1023d087e17a))

## [1.2.1](https://github.com/bettyblocks/wasm-base-components/compare/v1.2.0...v1.2.1) (2025-11-28)


### Bug Fixes

* type in path to crud component ([76bfad0](https://github.com/bettyblocks/wasm-base-components/commit/76bfad080119851b11452f5b12c4645483a972af))

# [1.2.0](https://github.com/bettyblocks/wasm-base-components/compare/v1.1.6...v1.2.0) (2025-11-26)


### Bug Fixes

* fix clippy findings and failing test ([9f8be8d](https://github.com/bettyblocks/wasm-base-components/commit/9f8be8d45c57e72ab5da3987e988c48b74dbb63e))


### Features

* add data api component helper ([919b893](https://github.com/bettyblocks/wasm-base-components/commit/919b893b4dee43363f73ba2fdd2447d0cb51ad0f))

## [1.1.6](https://github.com/bettyblocks/wasm-base-components/compare/v1.1.5...v1.1.6) (2025-11-06)


### Bug Fixes

* deep clone wadm to not overwrite origin ([3f637b1](https://github.com/bettyblocks/wasm-base-components/commit/3f637b168107739502104a484424e2c6e5d3dc1c))

## [1.1.5](https://github.com/bettyblocks/wasm-base-components/compare/v1.1.4...v1.1.5) (2025-11-06)


### Bug Fixes

* rename vabi to jenv url ([9c22ccf](https://github.com/bettyblocks/wasm-base-components/commit/9c22ccfecab843948afda46ff8e8d54f20163b57))

## [1.1.4](https://github.com/bettyblocks/wasm-base-components/compare/v1.1.3...v1.1.4) (2025-11-06)


### Bug Fixes

* rename acceptance to acc for deployment ([6967464](https://github.com/bettyblocks/wasm-base-components/commit/696746441c4b5f44fa336e2552851c53d8bfe5a4))

## [1.1.3](https://github.com/bettyblocks/wasm-base-components/compare/v1.1.2...v1.1.3) (2025-11-04)


### Bug Fixes

* automatically publish github packages and add environment name to ([d1d8143](https://github.com/bettyblocks/wasm-base-components/commit/d1d8143c48ce19b58bc36a1f758c56eb90143615))

## [1.1.2](https://github.com/bettyblocks/wasm-base-components/compare/v1.1.1...v1.1.2) (2025-11-04)


### Bug Fixes

* actually make the fetching of the version better ([bf3468b](https://github.com/bettyblocks/wasm-base-components/commit/bf3468b3d9731e4ae0d651e5d10658c569b8209f))
* generate jaws secret in correct format and make version more ([4de7b8a](https://github.com/bettyblocks/wasm-base-components/commit/4de7b8a40a67881a90c31b7422b41b75602db00e))
* put back the publish workflow ([5d085a9](https://github.com/bettyblocks/wasm-base-components/commit/5d085a96958d776122925b1b41849e7cad601f9d))
* remove putting the files into a release, just use the artifacts of ([86eacaa](https://github.com/bettyblocks/wasm-base-components/commit/86eacaabff5459e69d0ba5c77d5d93f6eb28b5b0))

## [1.1.1](https://github.com/bettyblocks/wasm-base-components/compare/v1.1.0...v1.1.1) (2025-11-03)


### Bug Fixes

* get deploy version via the logs ([5c5741e](https://github.com/bettyblocks/wasm-base-components/commit/5c5741e5a73e3dbbec4ff0ccdfafe973c7c433db))

# [1.1.0](https://github.com/bettyblocks/wasm-base-components/compare/v1.0.1...v1.1.0) (2025-11-03)


### Bug Fixes

* add jaws secret ([96413b3](https://github.com/bettyblocks/wasm-base-components/commit/96413b3b332edcab2ead508779c94877851f02ef))
* add publish to deploy workflow ([c55d187](https://github.com/bettyblocks/wasm-base-components/commit/c55d187bf12db23ba840e1dc88ae221ae99ff022))
* add two generate github secrets bun scripts ([2c77ced](https://github.com/bettyblocks/wasm-base-components/commit/2c77ced325c4202a856644b0f6d6d27661bf3713))
* allow to fetch elixir deps from betty_blocks_bv hex repo ([6abfc6e](https://github.com/bettyblocks/wasm-base-components/commit/6abfc6eb6c3da01a5060c8c8ba5950d13fe95d88))
* format the output correctly for deploy scripts ([7d4b734](https://github.com/bettyblocks/wasm-base-components/commit/7d4b7346f2f4ea546105018786621af9f8b24fe7))
* pin rust version in ci workflow ([d494537](https://github.com/bettyblocks/wasm-base-components/commit/d49453716019eb90b59458964658711bf48bc7a0))
* pin rust version to 1.88 in release workflow ([50a3d4f](https://github.com/bettyblocks/wasm-base-components/commit/50a3d4f74ba92e98cc243a7d77f5723b4bdc6e4c))
* use sync correct crud-component wit ([2432033](https://github.com/bettyblocks/wasm-base-components/commit/24320339678689dfc9c6e633dc38e30c406d6cc3))


### Features

* add first deploy script implementation ([57b77ca](https://github.com/bettyblocks/wasm-base-components/commit/57b77cabe6c3f0379571e8c3a2e3445800cf9ac6))
* put all providers into the same target folder and cache this ([57d7dcd](https://github.com/bettyblocks/wasm-base-components/commit/57d7dcdc3d57715970f7779df9bf5077095c8c7a))
* sync from native-wasm-components ([1be47d5](https://github.com/bettyblocks/wasm-base-components/commit/1be47d589ab42320d9a248cd6aaf66567dfc3963))

## [1.0.1](https://github.com/bettyblocks/wasm-base-components/compare/v1.0.0...v1.0.1) (2025-10-30)


### Bug Fixes

* add flag to allow push latest ([8d8ab5a](https://github.com/bettyblocks/wasm-base-components/commit/8d8ab5a10aeaf4d5d87c6dd9d469d5ee19a1aa2c))
* add the other wasm components as well using AI ([d58ed32](https://github.com/bettyblocks/wasm-base-components/commit/d58ed32e414fa36218fce4910bd112afdf36f9a6))
* include target folder in the to uploaded files ([233ca93](https://github.com/bettyblocks/wasm-base-components/commit/233ca938e254f9d28088a1db2bfce366b8efd6d1))

# 1.0.0 (2025-10-30)


### Bug Fixes

* add helpers to relaserc.json ([15c49b1](https://github.com/bettyblocks/wasm-base-components/commit/15c49b128b2177cc611ef03346cbc7fd1972121d))
* add protoc for data-api grpc api ([7098a9e](https://github.com/bettyblocks/wasm-base-components/commit/7098a9e137915bff5f3324c0805f64c33ec15709))
* use correct github url bettyblocks ([1c0c013](https://github.com/bettyblocks/wasm-base-components/commit/1c0c013a375fbb7bcf89c9d1889a2c5d0fe297e6))
* use public github jaws-rs ([5253603](https://github.com/bettyblocks/wasm-base-components/commit/52536036e5928c9de4bcb42644ee402f59a8b585))


### Features

* move actions-providers to github ([93256a6](https://github.com/bettyblocks/wasm-base-components/commit/93256a626c96b1a855292005176f7292dc361b3a))
* move helpers and providers from native-wasm-component repo ([0f68c92](https://github.com/bettyblocks/wasm-base-components/commit/0f68c92c5f6028d9530936333de32e654e21021c))
* try semantic release with the help of claude AI ([bac891a](https://github.com/bettyblocks/wasm-base-components/commit/bac891ad37eade749191b71b25ee0f483edcc940))
