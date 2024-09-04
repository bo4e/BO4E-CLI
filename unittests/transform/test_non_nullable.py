from bo4e_cli.models.schema import SchemaRootObject, String
from bo4e_cli.transform.non_nullable import field_to_non_nullable
from unittests.conftest import TEST_DATA_VERSION, TEST_DIR_BO4E_REL_REFS


class TestNonNullable:
    def test_field_to_non_nullable_with_default(self) -> None:
        angebot = SchemaRootObject.model_validate_json((TEST_DIR_BO4E_REL_REFS / "bo/Angebot.json").read_text())
        field_to_non_nullable(angebot, "_version")
        new_field = angebot.properties["_version"]

        assert isinstance(new_field, String)
        assert new_field.default == TEST_DATA_VERSION.to_str_without_prefix()
        assert "_version" not in angebot.required

    def test_field_to_non_nullable_without_default(self) -> None:
        angebot = SchemaRootObject.model_validate_json((TEST_DIR_BO4E_REL_REFS / "bo/Angebot.json").read_text())
        field_to_non_nullable(angebot, "angebotsdatum")
        new_field = angebot.properties["angebotsdatum"]

        assert isinstance(new_field, String)
        assert new_field.default is None
        assert "default" not in new_field.__pydantic_fields_set__
        assert new_field.format == "date-time"
