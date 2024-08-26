from pathlib import Path

from pydantic import TypeAdapter

from bo4e_cli.models.schema import AnyOf, Decimal, Reference, SchemaRootObject, SchemaRootStrEnum, SchemaRootType

TEST_DIR = Path(__file__).parents[1] / "test_data/bo4e_rel_refs"


class TestSchema:
    def test_angebot(self):
        with open(TEST_DIR / "bo/Angebot.json", "r") as f:
            schema = TypeAdapter(SchemaRootType).validate_json(f.read())
        expected_sparte_ref = Reference(ref="../enum/Sparte.json#")
        assert isinstance(schema, SchemaRootType)
        assert isinstance(schema, SchemaRootObject)
        assert schema.title == "Angebot"
        assert "_id" in schema.properties
        assert isinstance(schema.properties["sparte"], AnyOf)
        assert expected_sparte_ref in schema.properties["sparte"].any_of

    def test_decimal(self):
        with open(TEST_DIR / "bo/Zaehler.json", "r") as f:
            schema = TypeAdapter(SchemaRootType).validate_json(f.read())
        assert isinstance(schema, SchemaRootObject)
        assert isinstance(schema.properties["zaehlerkonstante"].any_of[0], Decimal)

    def test_enum(self):
        with open(TEST_DIR / "enum/Sparte.json", "r") as f:
            schema = TypeAdapter(SchemaRootType).validate_json(f.read())
        assert isinstance(schema, SchemaRootStrEnum)
        assert "STROM" in schema.enum
        assert "GAS" in schema.enum
