# BO4E-CLI

![Unittests status badge](https://github.com/bo4e/BO4E-CLI/actions/workflows/unittests.yml/badge.svg?branch=main)
![Coverage status badge](https://github.com/bo4e/BO4E-CLI/actions/workflows/coverage.yml/badge.svg?branch=main)
![Linting status badge](https://github.com/bo4e/BO4E-CLI/actions/workflows/pythonlint.yml/badge.svg?branch=main)
![Black status badge](https://github.com/bo4e/BO4E-CLI/actions/workflows/formatting.yml/badge.svg?branch=main)

This is a command line interface (CLI) for developers working or wanting to work with BO4E models.
It contains several features which can make your life easier when working with BO4E.

> It uses the [JSON-Schemas](https://github.com/bo4e/BO4E-Schemas) of the BO4E standard as source of truth.

## Features

- Pull JSON schemas of specific versions (or latest) conveniently from GitHub and replace the online
  references with relative paths.
- Edit JSON schemas using a static config file to customize the BO4E models to your usecase.
- Generate the models in one of the [supported languages](#supported-languages).
- Compare BO4E schemas of different versions. Create a machine-readable diff-file in json-format.
- Create a difference matrix comparing multiple versions consecutively by using multiple diff-files.
- Get information if a diff between two versions is functional or technical.

## Install

You need to have Python installed. Tested are Python versions `>3.10` but it may work for older versions too.

```bash
pip install bo4e-cli
```

Take a look at the CLI help:

```bash
bo4e --help
```

## Commands

In the following I will describe some details on how to use the functionality provided by this CLI. Please keep in
mind that I won't explain every command line option. In this regard, please refer to the help text of the CLI.
If you are missing something in the following explanation and/or the help text, feel free
to [create an issue](https://github.com/bo4e/BO4E-CLI/issues/new).

> Note: If you want to have a more verbose output, you have to place the option `-v` *before* the subcommand.
> I.e. you would have to write `bo4e -v pull -o ./bo4e_latest` or `bo4e -v diff version-bump ./diff.json`.

## `bo4e pull`

Pull all BO4E-JSON-schemas of a specific version (or `latest`).

Beside the json-files a `.version` file will be created in utf-8 format at root of the output directory.
This file is needed for other commands of this CLI.

The schemas pulled from the repository `BO4E-Schemas` contain online references to each other
(e.g. `"$ref": "https://raw.githubusercontent.com/BO4E/BO4E-Schemas/v202401.0.1/src/bo4e_schemas/bo/Angebot.json#"`).
This is not very convenient if you want to work with the schemas offline. And if you need to edit the schemas using
the config file, this would be a problem.

Per default (can be changed through command line option) the command replaces all online references
with relative references.

> Note: You might encounter rate limiting issues against the GitHub API. If true, please make use of a PAT. You can
> either provide it through the option `--token` or by setting the environment variable `GITHUB_ACCESS_TOKEN`
> or by having the GitHub CLI installed while being logged in. If a token wasn't provided either through the first or
> the second method, the CLI will automatically check for the GitHub CLI and if there is a user logged in.
> If true, the CLI will receive a temporary token by executing `gh auth token`.
>
> Also note that you don't need any special permissions behind this PAT. The GitHub API will increase the rate limit
> if the provided PAT is valid. If you are more interested in this, please refer to
>
the [GitHub documentation](https://docs.github.com/en/rest/using-the-rest-api/rate-limits-for-the-rest-api?apiVersion=2022-11-28)

Example:

```bash
bo4e pull -o ./bo4e_schemas_latest
```

## `bo4e edit`

In short, this lets you edit the schemas using a static config-file. Ideally, no one should need it but in
reality you might not have enough time to wait for the gremium or just want to experiment and elaborate
an appropriate model. Here is a list of what it can do:

- Define non-nullable properties (in most cases changes it to a required field)
- Add additional properties
- Add additional models
- Add additional enum values

### Config file

I think it's most effective to learn by example here:

```json
{
  "nonNullableFields": [
    "bo\\.Angebot\\.angebotspreis",
    "(bo|com)\\.\\w+\\._typ",
    "\\w+\\.\\w+\\._id"
  ],
  "additionalFields": [
    {
      "pattern": "bo\\.Angebot",
      "fieldName": "foo",
      "fieldDef": {
        "type": "number"
      }
    },
    {
      "$ref": "./models/bo/Geschaeftspartner_extension.json"
    }
  ],
  "additionalEnumItems": [
    {
      "pattern": "enum\\.BoTyp",
      "items": [
        "Bilanzierung",
        "Dokument"
      ]
    }
  ],
  "additionalModels": [
    {
      "module": "bo",
      "schema": {
        "$ref": "models/bo/Bilanzierung.json"
      }
    },
    {
      "module": "bo",
      "schema": {
        "additionalProperties": true,
        "title": "Dokument",
        "type": "object",
        "description": "A generic document reference like for bills, order confirmations and cancellations",
        "properties": {
          "boTyp": {
            "allOf": [
              {
                "$ref": "../enum/BoTyp.json#"
              }
            ],
            "title": "BoTyp",
            "default": "Dokument"
          },
          "erstellungsdatum": {
            "format": "date-time",
            "title": "Erstellungsdatum",
            "type": "string"
          }
        },
        "required": [
          "erstellungsdatum"
        ]
      }
    }
  ]
}
```

The config file can contain the following keys:

- `nonNullableFields`: A list of regex patterns which will be used to define non-nullable fields.
  The field will be required if the default value was `null`, which will be mostly the case.
  The regex pattern will be (full-)matched to the path of each field.
  An example of such a path would be `bo.Angebot.angebotspreis`. If the pattern matches, the field will be non-nullable.
- `additionalFields`: A list of additional fields which will be added to the schema.
    - `pattern`: A regex pattern which will be used to match the path of the schema (e.g. `bo.Angebot`).
      The field will be added to each schema to which the pattern matches.
    - `fieldName`: The name of the field which will be added.
    - `fieldDef`: The definition of the field which will be added.
- `additionalEnumItems`: A list of additional enum items which will be added to the schema.
    - `pattern`: A regex pattern which will be used to match the path of the enum (e.g. `enum.BoTyp`).
      The items will be added to each enum to which the pattern matches.
    - `items`: A list of items which will be added to the enum.
- `additionalModels`: A list of additional models which will be added to the schema.
    - `module`: The module to which the schema will be added.
    - `schema`: The schema definition which will be added.

Note: For all config keys (except for `nonNullableFields`), you can alternatively use the `"$ref"` key
to reference to a file.
This is useful to keep the config file small and to reuse definitions.
If the path is relative it will be applied to the path of the directory where the config file is stored in.
But, you can define absolute paths if you want.

As a little extra feature for `additionalFields`: If you want to define multiple fields in one external file,
you can define a list of fields instead of a single field. The reference in the `"$ref"` key is the same.

Example of `./models/bo/Geschaeftspartner_extension.json`:

```json
[
  {
    "pattern": "bo\\.Geschaeftspartner",
    "field_name": "foo",
    "field_def": {
      "type": "number"
    }
  },
  {
    "pattern": "bo\\.Geschaeftspartner",
    "field_name": "bar",
    "field_def": {
      "type": "string"
    }
  }
]
```

### Set Default Version

All BO4E-Schemas contain a field `_version` which defines the used BO4E version. All schemas which are pulled
from the repository [BO4E-Schemas](https://github.com/bo4e/BO4E-Schemas) will have the `_version` fields default value
set to the respective version.
But if you introduce additional models, it might be a bit cumbersome to set the `_version` field to the correct
version each time you upgrade the BO4E version.

To solve this problem, you can use the `--set-default-version` flag. It will automatically set or override the default
value for `_version` fields with the version inside the `.version` file.

Example:

```bash
bo4e edit -i ./bo4e_schemas_latest -o ./bo4e_schemas_edited -c ./my_config_file.json
```

## `bo4e generate`

This is a code-generation command. It creates all BO4E-models from the input JSON-schemas for a supported
output-type.

<a name="supported-languages"></a>Currently supported output types are:

- `python-pydantic-v1`: Programming language Python. Class definitions are
  in [pydantic](https://github.com/pydantic/pydantic) v1 style.
- `python-pydantic-v2`: Programming language Python. Class definitions are
  in [pydantic](https://github.com/pydantic/pydantic) v2 style.
- `python-sql-model`: Programming language Python. Class definitions are
  in [SQLModel](https://github.com/fastapi/sqlmodel) style.

Example:

```bash
bo4e generate -i ./bo4e_schemas_edited -o ./bo4e_schemas_python -t python-pydantic-v2
```

## `bo4e diff schemas`

Compares the JSON-schemas in the two input directories and saves the differences to the output file (JSON).
The output file will also contain information about the compared versions.

<details>
<summary>Here is an example of how this diff-file looks like</summary>

```json
{
  "old_schemas": {
    "schemas": [
      {
        "name": "Kundentyp",
        "module": [
          "enum",
          "Kundentyp"
        ],
        "src": "bo4e_latest\\enum\\Kundentyp.json"
      }
      // ...
    ],
    "version": {
      "major": 202501,
      "functional": 0,
      "technical": 0,
      "candidate": null,
      "commit_part": null,
      "dirty_workdir_date": null
    }
  },
  "new_schemas": {
    "schemas": [
      {
        "name": "Tarif",
        "module": [
          "bo",
          "Tarif"
        ],
        "src": "bo4e_latest\\bo\\Tarif.json"
      }
      // ...
    ],
    "version": {
      "major": 202501,
      "functional": 1,
      "technical": 0,
      "candidate": null,
      "commit_part": null,
      "dirty_workdir_date": null
    }
  },
  "changes": [
    {
      "type": "class_removed",
      "old": "bo\\AdditionalModel.json",
      "new": null,
      "old_trace": "/bo/AdditionalModel#",
      "new_trace": "/#"
    },
    {
      "type": "field_type_changed",
      "old": {
        "description": "Eine generische ID, die für eigene Zwecke genutzt werden kann.\nZ.B. könnten hier UUIDs aus einer Datenbank stehen oder URLs zu einem Backend-System.",
        "title": " Id",
        "default": null,
        "type": "string",
        "format": null
      },
      "new": {
        "description": "Eine generische ID, die für eigene Zwecke genutzt werden kann.\nZ.B. könnten hier UUIDs aus einer Datenbank stehen oder URLs zu einem Backend-System.",
        "title": " Id",
        "default": null,
        "any_of": [
          {
            "description": "",
            "title": "",
            "default": null,
            "type": "string",
            "format": null
          },
          {
            "description": "",
            "title": "",
            "default": null,
            "type": "null"
          }
        ]
      },
      "old_trace": "/com/Konzessionsabgabe#.properties['_id']",
      "new_trace": "/com/Konzessionsabgabe#.properties['_id']"
    }
    // ...
  ]
}
```

</details>

The type of change can be one of the following:

- `field_added`
- `field_removed`
- `field_default_changed`
- `field_description_changed`
- `field_title_changed`
- field type change types:
    - `field_cardinality_changed`
    - `field_reference_changed`
    - `field_string_format_changed`
    - `field_any_of_type_added`
    - `field_any_of_type_removed`
    - `field_all_of_type_added`
    - `field_all_of_type_removed`
    - `field_type_changed`  # An arbitrary unclassified change in type
- `class_added`
- `class_removed`
- `class_description_changed`
- `enum_value_added`
- `enum_value_removed`

Example:

```bash
bo4e diff schemas ./bo4e_schemas_v2024.0.0 ./bo4e_schemas_latest -o diff_v2024.0.0_to_latest.json
```

## `bo4e diff matrix`

This command creates a difference matrix just like
the [compatibility matrix](https://bo4e.github.io/BO4E-python/latest/changelog.html#compatibility) visible in the
documentation.

It uses multiple diff-files created by `bo4e diff schemas` where each file is represented by one column
in the resulting matrix. The diff-files will be ordered internally from earliest to latest version. So the order you
give the files as arguments doesn't matter. To make this work, the versions inside these diff files must be
consecutive and ascending. I.e. you have to be able to create an ascending series of versions where the `new_version`
must match the `old_version` of it's next neighbour. Example of valid input files:

| file 3                | &#8594; | file 1                | &#8594; | file 2                |
|-----------------------|---------|-----------------------|---------|-----------------------|
| v1.0.0 &#8594; v1.0.2 |         | v1.0.2 &#8594; v1.3.0 |         | v1.3.0 &#8594; v2.0.0 |

Example:

```bash
bo4e diff matrix diff_3.json diff_2.json diff_1.json -o matrix.csv -et csv
```

## `bo4e diff version-bump`

Given a diff file this command will decide if the list of changes corresponds to a functional or just technical change.
It will then take a look at the versions inside the file and will then print if the detected version bump is valid
or not. Alternatively, you can execute this command in `--quiet` mode in which case it will error with exit code `1`
if the version bump is invalid.

Example:

```bash
bo4e diff version-bump ./diff.json
```

## How to use this Repository on Your Machine

Follow the instructions in
our [Python template repository](https://github.com/Hochfrequenz/python_template_repository#how-to-use-this-repository-on-your-machine).

## Contribute

You are very welcome to contribute to this repository by opening a pull request against the main branch or by creating
an issue.
