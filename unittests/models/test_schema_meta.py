from pathlib import Path

import pytest

from bo4e_cli.models.meta import SchemaMeta


class TestSchemaMeta:
    def test_online_src(self):
        url = "https://raw.githubusercontent.com/BO4E/BO4E-Schemas/v202401.1.0-rc1/src/bo4e_schemas/bo/Angebot.json"
        schema_meta = SchemaMeta(
            name="Angebot",
            module=("bo", "Angebot"),
            src=url,
        )
        assert str(schema_meta.src_url) == url
        with pytest.raises(ValueError):
            _ = schema_meta.src_path

    def test_local_src(self):
        path = "src/bo4e_schemas/bo/Angebot.json"
        schema_meta = SchemaMeta(
            name="Angebot",
            module=("bo", "Angebot"),
            src=path,
        )
        assert schema_meta.src_path == Path(path)
        with pytest.raises(ValueError):
            _ = schema_meta.src_url
