from more_itertools import one

from bo4e_cli.io.config import load_config
from bo4e_cli.models.config import AdditionalField
from bo4e_cli.models.schema import Object
from bo4e_cli.transform.add import add_additional_property
from unittests.conftest import TEST_DIR, TEST_DIR_BO4E_REL_REFS


class TestAddProperty:
    def test_add_additional_property(self) -> None:
        angebot = Object.model_validate_json((TEST_DIR_BO4E_REL_REFS / "bo/Angebot.json").read_text())
        config = load_config(TEST_DIR / "config_test.json")
        angebot_field_foo = one(
            additional_field
            for additional_field in config.additional_fields
            if isinstance(additional_field, AdditionalField) and additional_field.field_name == "foo"
        )
        add_additional_property(angebot, angebot_field_foo.field_def, angebot_field_foo.field_name)

        assert "foo" in angebot.properties
        assert angebot.properties["foo"] == angebot_field_foo.field_def
