"""
Contains monkey patches related to imports
"""

import inspect
from collections import defaultdict
from typing import Iterable, List, Optional, Set, Union

from datamodel_code_generator.imports import Import
from datamodel_code_generator.imports import Imports as _Imports

from bo4e_cli.models.meta import Schemas


# pylint: disable=too-many-statements
def monkey_patch_imports(schemas: Schemas) -> None:
    """
    Overwrites the behaviour how imports are rendered. They are not going through jinja templates.
    They Imports class has a __str__ method, which we will overwrite here.
    """
    filtered_schemas = Schemas.model_validate(
        {"schemas": filter(lambda schema: schema.module[0] != "enum", schemas), "version": schemas.version}
    )
    # "Typ" and "Landescode" must not be wrapped inside the "if TYPE_CHECKING" block because they are used explicitly
    # to set default values. Generally, enums can be excluded since they cannot cause circular reference issues.
    import_type_checking = Import.from_full_path("typing.TYPE_CHECKING")

    # pylint: disable=missing-function-docstring
    class Imports(_Imports):
        """
        Re-implement some methods to customize the import rendering
        """

        def __str__(self) -> str:
            return self.dump()

        def _set_alias(self, from_: Optional[str], imports: Set[str]) -> List[str]:
            return [
                f"{i} as {self.alias[from_][i]}" if i in self.alias[from_] and i != self.alias[from_][i] else i
                for i in sorted(imports)
            ]

        def create_line(self, from_: Optional[str], imports: Set[str]) -> str:
            if from_:
                return f"from {from_} import {', '.join(self._set_alias(from_, imports))}"
            return "\n".join(f"import {i}" for i in self._set_alias(from_, imports))

        def dump(self) -> str:
            imports_type_checking = defaultdict(set)
            imports_no_type_checking = defaultdict(set)
            for from_, imports in self.items():
                for import_ in imports:
                    if import_ in filtered_schemas.names:
                        imports_type_checking[from_].add(import_)
                    else:
                        imports_no_type_checking[from_].add(import_)
            imports_dump = "\n".join(
                self.create_line(from_, imports) for from_, imports in imports_no_type_checking.items()
            )
            if len(imports_type_checking) > 0:
                imports_dump += "\n\n"
                imports_dump += "if TYPE_CHECKING:\n    "
                imports_dump += "\n    ".join(
                    self.create_line(from_, imports) for from_, imports in imports_type_checking.items()
                )
            return imports_dump

        def append(self, imports: Union[Import, Iterable[Import], None]) -> None:
            if imports:
                if isinstance(imports, Import):
                    imports = [imports]
                for import_ in imports:
                    if import_.reference_path:
                        self.reference_paths[import_.reference_path] = import_
                        if (
                            import_type_checking.from_ not in self
                            or import_type_checking.import_ not in self[import_type_checking.from_]
                        ):
                            self.append(import_type_checking)
                    if "." in import_.import_:
                        self[None].add(import_.import_)
                        self.counter[(None, import_.import_)] += 1
                    else:
                        self[import_.from_].add(import_.import_)
                        self.counter[(import_.from_, import_.import_)] += 1
                        if import_.alias:
                            self.alias[import_.from_][import_.import_] = import_.alias

        def remove(
            self, imports: Union[Import, Iterable[Import]], __intended_type_checking_remove: bool = False
        ) -> None:
            if isinstance(imports, Import):  # pragma: no cover
                imports = [imports]
            for import_ in imports:
                if not __intended_type_checking_remove and import_ == import_type_checking:
                    continue
                if "." in import_.import_:  # pragma: no cover
                    self.counter[(None, import_.import_)] -= 1
                    if self.counter[(None, import_.import_)] == 0:  # pragma: no cover
                        self[None].remove(import_.import_)
                        if not self[None]:
                            del self[None]
                else:
                    self.counter[(import_.from_, import_.import_)] -= 1  # pragma: no cover
                    if self.counter[(import_.from_, import_.import_)] == 0:  # pragma: no cover
                        self[import_.from_].remove(import_.import_)
                        if not self[import_.from_]:
                            del self[import_.from_]
                        if import_.alias:  # pragma: no cover
                            del self.alias[import_.from_][import_.import_]
                            if not self.alias[import_.from_]:
                                del self.alias[import_.from_]

                        if (
                            import_type_checking.from_ in self
                            and import_type_checking.import_ in self[import_type_checking.from_]
                            and not any(
                                imp_str in filtered_schemas.names
                                for imp_str_sets in self.values()
                                for imp_str in imp_str_sets
                            )
                        ):
                            self.remove(
                                import_type_checking,
                                __intended_type_checking_remove=True,  # type: ignore[call-arg]
                            )

        def remove_referenced_imports(self, reference_path: str) -> None:
            if reference_path in self.reference_paths:
                self.remove(self.reference_paths[reference_path])

    for name, func in inspect.getmembers(Imports, inspect.isfunction):
        setattr(_Imports, name, func)
