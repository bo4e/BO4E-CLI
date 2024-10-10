from more_itertools import one, take

from bo4e_cli.io.schemas import read_schemas
from bo4e_cli.models.meta import SchemaMeta, Schemas, Version
from unittests.conftest import TEST_DIR_BO4E_ORIGINAL


def get_disjoint_subsets(schemas: Schemas, no_subsets: int = 2) -> tuple[set[SchemaMeta], ...]:
    subsets = []
    iterator = iter(schemas.schemas)
    for _ in range(no_subsets):
        subsets.append(set(take(len(schemas) // no_subsets, iterator)))
    subsets[-1].update(iterator)
    return tuple(subsets)


class TestSchemas:
    def test_fields(self) -> None:
        schemas = read_schemas(TEST_DIR_BO4E_ORIGINAL)
        assert isinstance(schemas, Schemas)
        assert isinstance(schemas.version, Version)
        assert isinstance(schemas.schemas, set)
        assert all(isinstance(schema, SchemaMeta) for schema in schemas)

    def test_set_methods(self) -> None:
        schemas = read_schemas(TEST_DIR_BO4E_ORIGINAL)
        angebot_meta = one(schema for schema in schemas if schema.name == "Angebot")
        dummy_meta = SchemaMeta(name="dummy", module=("dummy",), src="dummy")  # type: ignore[arg-type]
        # pylint: disable=unbalanced-tuple-unpacking
        subset1, subset2, subset3 = get_disjoint_subsets(schemas, 3)

        assert angebot_meta in schemas
        assert len(schemas) == len(schemas.schemas)
        assert subset1.isdisjoint(subset2) and subset2.isdisjoint(subset3) and subset3.isdisjoint(subset1)
        assert schemas == subset1 | subset2 | subset3
        assert schemas != subset1
        assert all(schema in schemas for schema in schemas)

        assert schemas > subset1 and schemas > subset2 and schemas > subset3
        assert schemas >= subset1 and schemas >= subset2 and schemas >= subset3
        assert schemas - subset1 == subset2 | subset3
        assert schemas & subset1 == subset1
        subset1.add(dummy_meta)
        assert schemas ^ subset1 == subset2 | subset3 | {dummy_meta}
        subset1.remove(dummy_meta)

        schemas.add(dummy_meta)
        assert dummy_meta in schemas
        assert schemas ^ subset1 == subset2 | subset3 | {dummy_meta}
        schemas.remove(dummy_meta)
        assert dummy_meta not in schemas

        schemas.update(subset1, {dummy_meta})
        assert schemas == subset1 | subset2 | subset3 | {dummy_meta}

    def test_search_index_views(self) -> None:
        schemas = read_schemas(TEST_DIR_BO4E_ORIGINAL)
        cls_name_view = schemas.names
        angebot_meta = one(schema for schema in schemas if schema.name == "Angebot")

        assert len(cls_name_view) == len(schemas)
        assert cls_name_view[angebot_meta.name] == angebot_meta

        schemas.remove(angebot_meta)
        assert angebot_meta.name not in cls_name_view
        assert len(cls_name_view) == len(schemas)
        assert all(cls_name_view[name] in schemas for name in cls_name_view)
        assert cls_name_view.get("dummy") is None

        schemas.add(angebot_meta)
        assert cls_name_view == schemas.names
        assert cls_name_view != schemas.modules
        assert set(cls_name_view.values()) == schemas
        assert set(cls_name_view.keys()) == {schema.name for schema in schemas}
        assert set(cls_name_view.items()) == set(zip(cls_name_view.keys(), cls_name_view.values()))
