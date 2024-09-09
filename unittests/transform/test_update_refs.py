from pathlib import Path

from bo4e_cli.edit.update_refs import update_reference, update_references
from bo4e_cli.io.schemas import read_schemas
from bo4e_cli.models.meta import SchemaMeta, Schemas, Version
from bo4e_cli.models.schema import Object, Reference, SchemaRootObject, String

TEST_DIR = Path(__file__).parents[1] / "test_data/bo4e_original"


class TestUpdateRefs:
    def test_update_reference(self) -> None:
        schemas = read_schemas(TEST_DIR)
        angebot_meta = schemas.search_index_by_cls_name["Angebot"]
        example_ref = angebot_meta.get_schema_parsed().properties["_typ"].any_of[0]

        assert isinstance(example_ref, Reference)
        assert (
            example_ref.ref.lower() == f"https://raw.githubusercontent.com/BO4E/BO4E-Schemas/{schemas.version}/"
            f"src/bo4e_schemas/enum/Typ.json".lower()
        )
        update_reference(example_ref, angebot_meta, schemas)

        assert example_ref.ref == "../enum/Typ.json#"

    def test_update_references(self) -> None:
        schemas = read_schemas(TEST_DIR)
        angebot_meta = schemas.search_index_by_cls_name["Angebot"]
        update_references(angebot_meta, schemas)

        assert angebot_meta.get_schema_parsed().properties["_typ"].any_of[0].ref == "../enum/Typ.json#"
        assert angebot_meta.get_schema_parsed().properties["angebotsgeber"].any_of[0].ref == "Geschaeftspartner.json#"

    def test_update_reference_with_definitions(self) -> None:
        foo_schema = SchemaRootObject(properties={"bar": String(type="string")}, type="object")
        bar_schema = SchemaRootObject(
            defs={"Foo": Object(properties={"bar": String(type="string")}, type="object")},
            properties={"foo": Reference(ref="#/$defs/Foo")},
            type="object",
        )
        foo_meta = SchemaMeta(name="Foo", module=("com", "Foo"), src=Path())
        foo_meta.set_schema_text(foo_schema.model_dump_json())
        bar_meta = SchemaMeta(name="Bar", module=("bo", "Bar"), src=Path())
        bar_meta.set_schema_text(bar_schema.model_dump_json())

        update_references(bar_meta, Schemas(schemas={foo_meta, bar_meta}, version=Version.from_str("v200000.0.0")))

        assert bar_meta.get_schema_parsed().properties["foo"].ref == "../com/Foo.json#"
