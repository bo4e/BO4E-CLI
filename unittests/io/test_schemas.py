from pathlib import Path

from more_itertools import one

from bo4e_cli.io.schemas import read_schemas, write_schemas

from ..conftest import TEST_DIR


class TestSchemas:
    def test_read_schemas(self) -> None:
        # This is actually indirectly tested by many other tests, but here is one that is
        # specifically for this function.
        schemas = read_schemas(TEST_DIR)
        assert len(schemas) > 100
        assert schemas.version.major == 202401
        assert one(schema for schema in schemas if schema.name == "Angebot").get_schema_parsed().title == "Angebot"

    def test_write_schemas(self, tmp_path: Path) -> None:
        schemas = read_schemas(TEST_DIR)
        write_schemas(schemas, tmp_path)

        schemas_written = read_schemas(tmp_path)
        assert schemas_written.equals(schemas, "structure")
