## [1.0.1](https://github.com/vgerber/opage/compare/v1.0.0...v1.0.1) (2025-11-01)


### Bug Fixes

* Fixed dependencies overwritten by semverbot ([#5](https://github.com/vgerber/opage/issues/5)) ([8bff6bb](https://github.com/vgerber/opage/commit/8bff6bbdb6c2d35ac8492b5a0c2f1376adf1efee))

# 1.0.0 (2025-11-01)


### Bug Fixes

* Added missing one_of type check ([d1324fc](https://github.com/vgerber/opage/commit/d1324fcf14906e22b675287b61c37892cd156767))
* Added test openapi specs ([adc8bc0](https://github.com/vgerber/opage/commit/adc8bc0594ee3aa2a482d526b15e74fc52ce820d))
* Fixed components mod path ([1db62a1](https://github.com/vgerber/opage/commit/1db62a1576ab51474d4b01ccdafcba3629d71c00))
* Fixed creation of primitive components ([a6220e6](https://github.com/vgerber/opage/commit/a6220e683facdb04a3579b2d80e9dc227286cfc0))
* Fixed generation of websocket/x-stream endpoints ([68cbde7](https://github.com/vgerber/opage/commit/68cbde76420eb3f8ff638860ad01468100cb6285))
* Fixed keyword test ([d06bb38](https://github.com/vgerber/opage/commit/d06bb38bb772a35216ccaf6edb3aadd043509f37))
* Fixed name mapping resolve ([2fbf9e2](https://github.com/vgerber/opage/commit/2fbf9e2a5045487f6cb3f2042511bb6a5e3183ae))
* Fixed path output path ([b699be4](https://github.com/vgerber/opage/commit/b699be4c64acf3fae2b93acd8b8243f3600c03b0))
* Fixed test compilation ([239620d](https://github.com/vgerber/opage/commit/239620dc568533273d283dbb277154c3d00caf3e))
* Fixed wandelbots spec ([e29aa6f](https://github.com/vgerber/opage/commit/e29aa6f5cfab63fc7f5581bf6d5493ad5fadf1c7))
* Renamed package name ([0038c3a](https://github.com/vgerber/opage/commit/0038c3ac1f91a2cfc9f0201c24fa5decc84a0fd2))


### Features

* Added askama template support  ([#2](https://github.com/vgerber/opage/issues/2)) ([10c9215](https://github.com/vgerber/opage/commit/10c9215d1212141beaedb9d824359d0b58206c6d))
* Added basic websocket support ([ba54827](https://github.com/vgerber/opage/commit/ba54827bcf9e1f4e77aa2b065e45faaf3064a9be))
* Added cli interface ([1b0b2b5](https://github.com/vgerber/opage/commit/1b0b2b500212374a5d32fbd4820720292e0f42e5))
* Added dummy cargo file + tests ([91eb181](https://github.com/vgerber/opage/commit/91eb18132b5439be8ff3c25edc11ba6702bf5d5c))
* Added fallabck to string if const or just a default value ([af4d9cc](https://github.com/vgerber/opage/commit/af4d9cc8fa817b3cab5cc49a18231b2a475cad1a))
* Added fork of oas3 ([3a1cc98](https://github.com/vgerber/opage/commit/3a1cc98b338d4f2d6b32bdc767c5a68f9e243bff))
* Added logging and renaming based on object path ([d2612c5](https://github.com/vgerber/opage/commit/d2612c5a0a5b04c764b417f6598f003579ae1fba))
* Added support for empty json objects ([537c5d1](https://github.com/vgerber/opage/commit/537c5d162972b20b7a55d093d5c953ca97641e44))
* Added support for enums/anyTypes ([f02e726](https://github.com/vgerber/opage/commit/f02e7268c05a4c42792181625a80add1197f3465))
* Added support for multi content type responses ([0badcad](https://github.com/vgerber/opage/commit/0badcad17fd5877e59dd2eda66b14545dd50e8c0))
* Added support for multi content-type requests ([7f20cd2](https://github.com/vgerber/opage/commit/7f20cd282f605a9ae7d08a2476a0c82ba409dd72))
* Added support for primitive type components ([8063d38](https://github.com/vgerber/opage/commit/8063d381dadc0bbabe65220f6bca5ac310762b68))
* Added text/plain support ([8345753](https://github.com/vgerber/opage/commit/8345753df1da46df8053f6cf547ccb701da9bcb0))
* Switched status code  check to u16 from str ([bdfe704](https://github.com/vgerber/opage/commit/bdfe704a7400c307d46782529ba3047999594fdc))
