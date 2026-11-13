#include "asset_converter.h"
#include <cassert>
#include <cstdint>
#include <iostream>
#include <vector>
#include "ragekit/ragekit.h"
int main() {
    rk::Texture texture;
    texture.name = "converter_smoke";
    texture.width = 4;
    texture.height = 4;
    texture.levels = 1;
    texture.format = rk::TextureFormat::DXT1;
    texture.data.assign(8, 0x5A);
    rk::TextureDictionary dictionary;
    dictionary.add(texture);
    const auto gen9 = rk::write_ytd(dictionary, rk::Game::Enhanced);
    assert(rk::rsc::Resource::load(gen9).version == 5);
    gta_asset_converter::ConversionStats stats;
    const auto legacy = gta_asset_converter::convert_asset_bytes("YTD", gen9, stats);
    assert(rk::rsc::Resource::load(legacy).version == 13);
    assert(stats.converted_ytd == 1);
    const auto parsed = rk::read_ytd(legacy);
    assert(parsed.textures.size() == 1);
    assert(parsed.textures[0].name == "converter_smoke");
    assert(parsed.textures[0].data == texture.data);
    const auto unchanged = gta_asset_converter::convert_asset_bytes("ytd", legacy, stats);
    assert(unchanged == legacy);
    assert(stats.already_legacy == 1);
    const std::vector<std::uint8_t> opaque = {1, 2, 3, 4};
    const auto passthrough = gta_asset_converter::convert_asset_bytes("dat", opaque, stats);
    assert(passthrough == opaque);
    assert(stats.unchanged_other == 1);
    std::cout << "Gen9 YTD -> Legacy YTD conversion smoke test passed\n";
    return 0;
}
