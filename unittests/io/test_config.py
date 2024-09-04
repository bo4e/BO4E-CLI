from bo4e_cli.io.config import load_config
from bo4e_cli.models.config import Config
from unittests.conftest import TEST_DIR


class TestConfig:
    def test_load_config(self) -> None:
        _ = load_config(TEST_DIR / "config_test.json")

    def test_config_optional_fields(self) -> None:
        _ = Config.model_validate({})
