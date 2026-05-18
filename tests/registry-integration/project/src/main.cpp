#include <iostream>
#include <string>
#include <nlohmann/json.hpp>
#include <sqlite3.h>

// ── nlohmann/json ─────────────────────────────────────────────────────────────

static void test_json() {
    auto j = nlohmann::json::parse(
        R"({"project":"freight","stars":42,"tags":["build","cpp","toml"]})"
    );

    std::string name  = j["project"];
    int         stars = j["stars"];
    std::string tag0  = j["tags"][0];

    std::cout << "json.project: " << name  << "\n";
    std::cout << "json.stars:   " << stars << "\n";
    std::cout << "json.tags[0]: " << tag0  << "\n";

    // Round-trip: serialise and re-parse
    auto j2 = nlohmann::json::parse(j.dump());
    if (j2 != j) { std::cerr << "json round-trip FAILED\n"; std::exit(1); }
}

// ── SQLite ────────────────────────────────────────────────────────────────────

static void test_sqlite() {
    sqlite3 *db = nullptr;
    if (sqlite3_open(":memory:", &db) != SQLITE_OK) {
        std::cerr << "sqlite3_open FAILED\n"; std::exit(1);
    }

    const char *ddl =
        "CREATE TABLE packages(name TEXT, version TEXT);"
        "INSERT INTO packages VALUES('nlohmann-json','3.11.3');"
        "INSERT INTO packages VALUES('sqlite3','3.45.1');";
    char *errmsg = nullptr;
    sqlite3_exec(db, ddl, nullptr, nullptr, &errmsg);
    if (errmsg) {
        std::cerr << "DDL error: " << errmsg << "\n";
        sqlite3_free(errmsg);
        std::exit(1);
    }

    sqlite3_stmt *stmt = nullptr;
    sqlite3_prepare_v2(db,
        "SELECT name, version FROM packages ORDER BY name",
        -1, &stmt, nullptr);

    while (sqlite3_step(stmt) == SQLITE_ROW) {
        std::string n = reinterpret_cast<const char *>(sqlite3_column_text(stmt, 0));
        std::string v = reinterpret_cast<const char *>(sqlite3_column_text(stmt, 1));
        std::cout << "sqlite " << n << ": " << v << "\n";
    }
    sqlite3_finalize(stmt);

    // Aggregate query
    sqlite3_prepare_v2(db, "SELECT COUNT(*) FROM packages", -1, &stmt, nullptr);
    sqlite3_step(stmt);
    int count = sqlite3_column_int(stmt, 0);
    std::cout << "sqlite.count: " << count << "\n";
    sqlite3_finalize(stmt);

    sqlite3_close(db);
}

// ── main ──────────────────────────────────────────────────────────────────────

int main() {
    test_json();
    test_sqlite();
    std::cout << "PASS\n";
}
