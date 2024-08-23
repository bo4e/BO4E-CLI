class MagicForwardMixin:
    def __getattr__(self, item):
        if item.startswith("__") and item.endswith("__") and self.__:
            raise AttributeError(item)
        return self[item]
