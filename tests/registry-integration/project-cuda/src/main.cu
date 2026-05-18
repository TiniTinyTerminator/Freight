// Compiled by nvcc — host-only CUDA source (no __global__ kernels, no cudaXxx calls).
// Because there is no device code, nvcc does not embed a fat binary and does not
// add CUDA-runtime initialisation stubs, so the binary requires no libcudart at
// runtime.  This is what "sim mode" means here: the CUDA compilation pipeline is
// exercised without needing a physical GPU.
//
// __CUDACC_VER_MAJOR__ / MINOR__ are defined by nvcc at compile time.

#include <nlohmann/json.hpp>
#include <sqlite3.h>
#include <cstdio>
#include <string>

// ── JSON test ─────────────────────────────────────────────────────────────────

static void test_json() {
    using json = nlohmann::json;
    auto j = json::parse(R"({
        "project": "freight",
        "stars": 42,
        "tags": ["build", "cpp", "freight"]
    })");
    printf("json.project: %s\n", j["project"].get<std::string>().c_str());
    printf("json.stars:   %d\n", j["stars"].get<int>());
    printf("json.tags[0]: %s\n", j["tags"][0].get<std::string>().c_str());
}

// ── SQLite test ───────────────────────────────────────────────────────────────

static void test_sqlite() {
    sqlite3 *db = nullptr;
    sqlite3_open(":memory:", &db);
    sqlite3_exec(db,
        "CREATE TABLE packages(name TEXT, version TEXT);"
        "INSERT INTO packages VALUES('nlohmann-json','3.11.3');"
        "INSERT INTO packages VALUES('sqlite3','3.45.1');",
        nullptr, nullptr, nullptr);

    sqlite3_stmt *stmt = nullptr;
    sqlite3_prepare_v2(db,
        "SELECT name, version FROM packages ORDER BY name", -1, &stmt, nullptr);
    while (sqlite3_step(stmt) == SQLITE_ROW) {
        printf("sqlite %s: %s\n",
               sqlite3_column_text(stmt, 0),
               sqlite3_column_text(stmt, 1));
    }
    sqlite3_finalize(stmt);

    int count = 0;
    sqlite3_prepare_v2(db, "SELECT COUNT(*) FROM packages", -1, &stmt, nullptr);
    if (sqlite3_step(stmt) == SQLITE_ROW) count = sqlite3_column_int(stmt, 0);
    sqlite3_finalize(stmt);
    sqlite3_close(db);

    printf("sqlite.count: %d\n", count);
}

// ── Main ──────────────────────────────────────────────────────────────────────

int main() {
    test_json();
    test_sqlite();
    // Report the nvcc version that compiled this file (compile-time constant, no
    // CUDA runtime call — safe to run without a GPU).
    printf("cuda.nvcc:    %d.%d\n",
           __CUDACC_VER_MAJOR__, __CUDACC_VER_MINOR__);
    printf("PASS\n");
    return 0;
}
