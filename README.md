# BO4E-CLI

![Unittests status badge](https://github.com/bo4e/BO4E-CLI/actions/workflows/unittests.yml/badge.svg?branch=main)
![Coverage status badge](https://github.com/bo4e/BO4E-CLI/actions/workflows/coverage.yml/badge.svg?branch=main)
![Linting status badge](https://github.com/bo4e/BO4E-CLI/actions/workflows/pythonlint.yml/badge.svg?branch=main)
![Black status badge](https://github.com/bo4e/BO4E-CLI/actions/workflows/formatting.yml/badge.svg?branch=main)

This is a command line interface (CLI) for developers working or wanting to work with BO4E models.
It contains several features which can make your life easier when working with BO4E.

> It uses the [JSON-Schemas](https://github.com/bo4e/BO4E-Schemas) of the BO4E standard as source of truth.

## Features

- Pull JSON schemas of specific version conveniently from GitHub and replace the online references with relative paths.
- Edit JSON schemas using a static config file to customize the BO4E models to your usecase.
- Generate the models in one of the supported languages: Python (pydantic v1, pydantic v2, sql-model).
- Compare BO4E schemas of different versions. Creates machine-readable output.
  - Create a difference matrix comparing multiple versions consecutively.
  - Decide if a Diff between two versions is functional or technical.

## How to use this Repository on Your Machine

Follow the instructions in our [Python template repository](https://github.com/Hochfrequenz/python_template_repository#how-to-use-this-repository-on-your-machine).

## Contribute

You are very welcome to contribute to this repository by opening a pull request against the main branch.
