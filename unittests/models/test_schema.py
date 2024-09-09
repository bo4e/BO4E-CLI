from pydantic import TypeAdapter

from bo4e_cli.models.schema import AnyOf, Decimal, Reference, SchemaRootObject, SchemaRootStrEnum, SchemaRootType
from unittests.conftest import TEST_DIR_BO4E_REL_REFS


class TestSchema:
    def test_angebot(self) -> None:
        with open(TEST_DIR_BO4E_REL_REFS / "bo/Angebot.json", "r", encoding="utf-8") as f:
            schema: SchemaRootType = TypeAdapter(SchemaRootType).validate_json(f.read())
        expected_sparte_ref = Reference(ref="../enum/Sparte.json#")
        assert isinstance(schema, SchemaRootObject)
        assert schema.title == "Angebot"
        assert "_id" in schema.properties
        assert isinstance(schema.properties["sparte"], AnyOf)
        assert expected_sparte_ref in schema.properties["sparte"].any_of

    def test_decimal(self) -> None:
        with open(TEST_DIR_BO4E_REL_REFS / "bo/Zaehler.json", "r", encoding="utf-8") as f:
            schema: SchemaRootType = TypeAdapter(SchemaRootType).validate_json(f.read())
        assert isinstance(schema, SchemaRootObject)
        assert isinstance(schema.properties["zaehlerkonstante"].any_of[0], Decimal)

    def test_enum(self) -> None:
        with open(TEST_DIR_BO4E_REL_REFS / "enum/Sparte.json", "r", encoding="utf-8") as f:
            schema: SchemaRootType = TypeAdapter(SchemaRootType).validate_json(f.read())
        assert isinstance(schema, SchemaRootStrEnum)
        assert "STROM" in schema.enum
        assert "GAS" in schema.enum
