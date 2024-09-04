from typing import Iterable

from bo4e_cli.models.meta import SchemaMeta
from bo4e_cli.models.schema import Object


def get_all_field_paths_from_schema(schema: SchemaMeta) -> Iterable[tuple[str, str]]:
    """
    Get all field paths of the schema.
    Returns an iterable of tuples with the field path and the field name.
    An element could be e.g. ("bo.Angebot.angebotsnehmer", "angebotsnehmer").
    """
    if not isinstance(schema.get_schema_parsed(), Object):
        return
    for field_name in schema.get_schema_parsed().properties:
        yield ".".join((*schema.module, field_name)), field_name
