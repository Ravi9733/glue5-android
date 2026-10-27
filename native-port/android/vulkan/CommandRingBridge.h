#pragma once

#include <cstddef>
#include <cstdint>

namespace mugoat::vkhost {

// A read-only, bounds-checked view of one packet in Mugoat's shared GPU ring.
// The renderer owns command interpretation; this class never mutates ring
// counters or claims that a packet has been rendered.
enum class PacketStatus : uint8_t { Ready, Empty, Wrap, Invalid };

struct CommandPacket {
  uint32_t opcode = 0;
  uint32_t payload_words = 0;
  uint32_t offset = 0;
  uint32_t end_offset = 0;
  const uint32_t* payload = nullptr;
};

class CommandRingReader {
 public:
  CommandRingReader(const void* memory, size_t memory_size, size_t ring_base,
                    uint32_t tail, uint32_t head, uint32_t ring_size);

  PacketStatus peek(CommandPacket* out) const;
  bool consume(const CommandPacket& packet);
  // Advance past an explicit WRAP marker to the first packet after the ring header.
  bool consumeWrap();
  uint32_t tail() const { return tail_; }

 private:
  const uint8_t* memory_ = nullptr;
  size_t memory_size_ = 0;
  size_t ring_base_ = 0;
  uint32_t tail_ = 0;
  uint32_t head_ = 0;
  uint32_t ring_size_ = 0;
  bool valid_ = false;
};

}  // namespace mugoat::vkhost
