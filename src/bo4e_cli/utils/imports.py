"""
Contains utility functions related to imports.
Used mainly in the Python code generation.
"""

from datamodel_code_generator.imports import Import
from datamodel_code_generator.parser import base as dcg_base


def relative_import(cur_module: str, reference: str) -> Import:
    """
    Create a relative import from the current module to the reference. The reference can be a module or a class inside
    a module.
    """
    from_, import_ = dcg_base.relative(cur_module, reference)
    return Import(from_=from_, import_=import_)
