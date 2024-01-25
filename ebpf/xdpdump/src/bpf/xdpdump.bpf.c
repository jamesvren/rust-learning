#include "vmlinux.h"
#include <bpf/bpf_helpers.h>
#include <bpf/bpf_endian.h>

char __license[] SEC("license") = "GPL";

struct {
  __uint(type, BPF_MAP_TYPE_HASH);
  __uint(max_entries, 100000);
  __type(key, u32);
  __type(value, u32);
} filterlist SEC(".maps");

struct arp_t {
  unsigned short htype;
  unsigned short ptype;
  unsigned char  hlen;
  unsigned char  plen;
  unsigned short oper;
  unsigned long long sha:48;
  unsigned long long spa:32;
  unsigned long long tha:48;
  unsigned int       tpa;
} __attribute__((packed));

SEC("xdp")
int xdp_dump(struct xdp_md *ctx)
{
  void *data = (void *)(long)ctx->data;
  void *data_end = (void *)(long)ctx->data_end;
  int pkt_sz = data_end - data;

  u32 ip_src;
  u64 *value;
  struct ethhdr *eth = data;

  if (pkt_sz <= (sizeof(struct ethhdr) + sizeof(struct arp_t))) {
    return XDP_PASS;
  }

  if (data_end < data + sizeof(*eth)) {
    return XDP_PASS;
  }
  if (eth->h_proto != bpf_htons(0x0806)) {
    return XDP_PASS;
  }

  struct arp_t *arp = data + sizeof(*eth);
  if (data_end < data + sizeof(*eth) + sizeof(*arp)) {
    return XDP_PASS;
  }

  ip_src = arp->tpa;
  value = bpf_map_lookup_elem(&filterlist, &ip_src);
  if (value) {
    bpf_printk("ARP packet from %d, size %d, value %d", ip_src, pkt_sz, value);
  }

  return XDP_PASS;
}
