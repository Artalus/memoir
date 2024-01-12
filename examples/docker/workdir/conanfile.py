from conan import ConanFile

class Pkg(ConanFile):
    name = "memoir"

    def requirements(self):
        self.requires("boost/1.83.0")
