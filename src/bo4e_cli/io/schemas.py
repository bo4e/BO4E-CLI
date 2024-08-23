from pathlib import Path

from rich.progress import track

from bo4e_cli.models.github import SchemaMeta, Schemas


def write_schemas(schemas: Schemas, output_dir: Path) -> None:
    """
    Write the schemas to the output directory.
    """
    for schema in track(schemas, description="Writing schemas...", total=len(schemas)):
        (output_dir / schema.relative_path).write_text(schema.schema_text, encoding="utf-8")


def read_schemas(output_dir: Path) -> Schemas:
    """
    Read the schemas from the output directory.
    """
    schemas = Schemas()
    all_files = list(output_dir.rglob("*.json"))
    for schema_path in track(all_files, description="Reading schemas...", total=len(all_files)):
        relative_path = schema_path.relative_to(output_dir).with_suffix("")
        schema = SchemaMeta(name=schema_path.name, module=relative_path.parts)
        schema.schema_text = schema_path.read_text(encoding="utf-8")
        schemas.add(schema)
    return schemas
