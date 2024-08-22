import re

from bo4e_cli.io.github import OWNER, REPO
from bo4e_cli.models.schema import Reference, SchemaRootType


def parse_schema(schema: str) -> SchemaRootType:
    """Parse a JSON schema string into a pydantic model."""
    return SchemaRootType.model_validate_json(schema)


# GH_VERSION_REGEX = re.compile(r"^v(\d+\.\d+\.\d+)(-rc\d+)?$")
REF_ONLINE_REGEX = re.compile(
    rf"^https://raw\.githubusercontent\.com/(?:{OWNER.upper()}|{OWNER.lower()}|Hochfrequenz)/{REPO}/"
    r"(?P<version>v\d+\.\d+\.\d+(?:-rc\d+)?)/"
    r"src/bo4e_schemas/(?P<sub_path>(?:\w+/)*)(?P<model>\w+)\.json#?$"
)
# e.g. https://raw.githubusercontent.com/BO4E/BO4E-Schemas/v202401.1.0-rc1/src/bo4e_schemas/bo/Angebot.json
REF_DEFS_REGEX = re.compile(r"^#/\$(?:defs|definitions)/(?P<model>\w+)$")


def update_reference(
    field: Reference,
    schema: SchemaRootType,
    module_path: tuple[str, ...],
    schemas: dict[str, SchemaMetadata],
    version: str,
):
    """
    Update a reference to a schema file by replacing a URL reference or reference to definitions with a relative path
    to the schema file. If using references to definitions, the schema file must be in the namespace.
    Example of online reference:
    https://raw.githubusercontent.com/BO4E/BO4E-Schemas/v202401.1.0-rc1/src/bo4e_schemas/bo/Angebot.json
    Example of reference to definitions:
    #/$defs/Angebot
    """
    match = REF_ONLINE_REGEX.search(field.ref)
    if match is not None:
        logger.debug("Matched online reference: %s", field.ref)
        if match.group("version") != version:
            raise ValueError(
                "Version mismatch: References across different versions of BO4E are not allowed. "
                f"{match.group('version')} does not match {version} for reference {field.ref}"
            )
        if match.group("sub_path") is not None:
            reference_module_path = [*match.group("sub_path").split("/")[:-1], match.group("model")]
        else:
            reference_module_path = [match.group("model")]
    else:
        match = REF_DEFS_REGEX.search(field.ref)
        if match is not None:
            logger.debug("Matched reference to definitions: %s", field.ref)
            if match.group("model") not in schemas:
                raise ValueError(
                    f"Could not find schema for reference {field.ref} in namespace "
                    f"{set(schema_el.module_path for schema_el in schemas.values())}"
                )
            reference_module_path = list(schemas[match.group("model")].module_path)
        else:
            logger.info("Reference unchanged. Could not parse reference: %s", field.ref)
            return

    relative_ref = "#"
    for ind, (part, own_part) in enumerate(zip(reference_module_path, schema.module_path)):
        if part != own_part:
            relative_ref = (
                "../" * (len(schema.module_path) - ind - 1) + "/".join(reference_module_path[ind:]) + ".json#"
            )
            break

    logger.debug("Updated reference %s to: %s", field.ref, relative_ref)
    field.ref = relative_ref


def update_references(schema: SchemaMetadata, schemas: dict[str, SchemaMetadata], version: str):
    """
    Update all references in a schema object. Iterates through the whole structure and calls `update_reference`
    on every Reference object.
    """

    def update_or_iter(_object: SchemaType):
        if isinstance(_object, Object):
            iter_object(_object)
        elif isinstance(_object, AnyOf):
            iter_any_of(_object)
        elif isinstance(_object, AllOf):
            iter_all_of(_object)
        elif isinstance(_object, Array):
            iter_array(_object)
        elif isinstance(_object, Reference):
            update_reference(_object, schema, schemas, version)

    def iter_object(_object: Object):
        for prop in _object.properties.values():
            update_or_iter(prop)

    def iter_any_of(_object: AnyOf):
        for item in _object.any_of:
            update_or_iter(item)

    def iter_all_of(_object: AllOf):
        for item in _object.all_of:
            update_or_iter(item)

    def iter_array(_object: Array):
        update_or_iter(_object.items)

    update_or_iter(schema.schema_parsed)
