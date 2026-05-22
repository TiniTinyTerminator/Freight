#include <fmt/core.h>           // registry dep: fmt 10.2.0
#include <nlohmann/json.hpp>    // registry dep: nlohmann-json 3.10.2

#include <string>
#include <vector>

struct Package {
    std::string name;
    std::string version;
    std::vector<std::string> deps;
};

int main() {
    std::vector<Package> packages = {
        { "fmt",           "10.2.0", {} },
        { "nlohmann-json", "3.10.2", {} },
        { "zlib",          "1.3.2",  { "vcpkg-cmake", "vcpkg-cmake-config" } },
        { "openssl",       "3.3.0",  { "zlib" } },
    };

    // Build a JSON object using nlohmann-json and render it with fmt.
    nlohmann::json registry;
    for (const auto& pkg : packages) {
        nlohmann::json entry;
        entry["version"] = pkg.version;
        if (!pkg.deps.empty()) {
            entry["dependencies"] = pkg.deps;
        }
        registry[pkg.name] = entry;
    }

    fmt::print("Freight registry snapshot ({} packages):\n\n", packages.size());
    fmt::print("{}\n", registry.dump(2));

    // Show how version constraints would be written in freight.toml.
    fmt::print("\nEquivalent freight.toml [dependencies]:\n");
    for (const auto& pkg : packages) {
        fmt::print("  {:25s} = \"{}\"\n", pkg.name, pkg.version);
    }

    return 0;
}
