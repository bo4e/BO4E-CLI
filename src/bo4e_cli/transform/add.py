import re

from bo4e_cli.models.config import AdditionalEnumItem, AdditionalField
from bo4e_cli.models.meta import Schemas
from bo4e_cli.models.schema import Object, SchemaType, StrEnum


def add_additional_property(obj: Object, additional_property: SchemaType, property_name: str) -> Object:
    """
    Add a property to an object.
    """
    obj.properties[property_name] = additional_property
    return obj


def add_additional_enum_items(obj: StrEnum, additional_items: list[str]) -> StrEnum:
    """
    Add items to an enum.
    """
    obj.enum.extend(additional_items)
    return obj


def transform_all_additional_fields(additional_fields: list[AdditionalField], schemas: Schemas):
    """
    Apply the additional field patterns to all schemas and adds the respective field definition.
    """
    for additional_field in additional_fields:
        compiled_pattern = re.compile(additional_field.pattern)
        matches = 0
        for schema in schemas:
            schema_path = ".".join(schema.module)
            if compiled_pattern.fullmatch(schema_path) and isinstance(schema.schema_parsed, Object):
                matches += 1
                add_additional_property(schema.schema_parsed, additional_field.field_def, additional_field.field_name)

                if (
                    "default" not in additional_field.field_def.__pydantic_fields_set__
                    and additional_field.field_name not in schema.schema_parsed.required
                ):
                    if "required" not in schema.schema_parsed.__pydantic_fields_set__:
                        schema.schema_parsed.required = []
                    schema.schema_parsed.required.append(additional_field.field_name)
                # logger.info(
                #     "Applied pattern '%s' to schema %s. Added field %s",
                #     additional_field.pattern,
                #     schema_path,
                #     additional_field.field_name,
                # )
        # if matches == 0:
        #     logger.warning("Pattern '%s' did not match any fields", additional_field.pattern)
        # else:
        #     logger.info("Pattern '%s' matched %d fields", additional_field.pattern, matches)


def transform_all_additional_enum_items(additional_enum_items: list[AdditionalEnumItem], schemas: Schemas):
    """
    Apply the additional enum item patterns to all schemas and adds the respective enum items.
    """
    for additional_item in additional_enum_items:
        compiled_pattern = re.compile(additional_item.pattern)
        matches = 0
        for schema in schemas:
            schema_path = ".".join(schema.module)
            if compiled_pattern.fullmatch(schema_path) and isinstance(schema.schema_parsed, StrEnum):
                matches += 1
                add_additional_enum_items(schema.schema_parsed, additional_item.items)
        #         logger.info(
        #             "Applied pattern '%s' to schema %s. Added enum items %s",
        #             additional_item.pattern,
        #             schema_path,
        #             str(additional_item.items),
        #         )
        # if matches == 0:
        #     logger.warning("Pattern '%s' did not match any fields", additional_item.pattern)
        # else:
        #     logger.info("Pattern '%s' matched %d fields", additional_item.pattern, matches)
