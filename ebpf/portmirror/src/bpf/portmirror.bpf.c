#include "vmlinux.h"
#include <bpf/bpf_helpers.h>
#include <bpf/bpf_endian.h>

#define TC_ACT_OK            0
#define TC_ACT_REDIRECT      7

#define MIRROR 1

#define ETH_P_IP     0x0800
#define ETH_P_IPV6   0x86DD

char __license[] SEC("license") = "GPL";


#define HASH_TYPE_NONE           0x00
#define HASH_TYPE_SRC            0x01
#define HASH_TYPE_DST            0x02
#define HASH_TYPE_PROTO          0x04
#define HASH_TYPE_SPORT          0x08
#define HASH_TYPE_DPORT          0x10

const volatile __u8 hash_type = 0;

struct flow_key {
    union {
        __u32 saddr;
        __u32 saddr6[4];
    };
    union {
        __u32 daddr;
        __u32 daddr6[4];
    };
    __u16 sport;
    __u16 dport;
    __u8 proto;
    __u8 ip_version;
} __attribute__((packed));
//};

struct {
    __uint(type, BPF_MAP_TYPE_HASH);
    __uint(max_entries, 1024);
    __type(key, struct flow_key);
    __type(value, __u8);    // 1: Mirror, other: Pass
} filter_map SEC(".maps");

struct {
    __uint(type, BPF_MAP_TYPE_HASH);
    __uint(max_entries, 1024);
    __type(key, __u32);    // TAP interface index
    __type(value, __u32);  // Mirror interface index
} mirror_map SEC(".maps");

#define log_err(__ret, __msg) bpf_printk("ERROR line:%d ret:%d error: %s\n", __LINE__, __ret, __msg)


//static __always_inline int parse_ipv6(struct __sk_buff *skb, struct flow_key *key) {
//    void *data_end = (void *)(long)skb->data_end;
//    struct ipv6hdr *ip6h = (struct ipv6hdr *)(skb->data + sizeof(struct ethhdr));
//    if ((void *)(ip6h + 1) > data_end)
//        return TC_ACT_OK;
//
//    __builtin_memcpy(key->saddr6, ip6h->saddr.s6_addr, 16);
//    __builtin_memcpy(key->daddr6, ip6h->daddr.s6_addr, 16);
//    key->ip_version = 6;
//
//    if (ip6h->nexthdr == IPPROTO_TCP || ip6h->nexthdr == IPPROTO_UDP) {
//        struct tcphdr *tcp = (struct tcphdr *)(ip6h + 1);
//        if ((void *)(tcp + 1) > data_end)
//            return TC_ACT_OK;
//
//        key->sport = bpf_ntohs(tcp->source);
//        key->dport = bpf_ntohs(tcp->dest);
//        key->proto = ip6h->nexthdr;
//    }
//    return 0;
//}

SEC("tc")
int port_mirror(struct __sk_buff *skb)
{
    void *data = (void *)(long)skb->data;
    void *data_end = (void *)(long)skb->data_end;

    bpf_printk("HASH TYPE: %u", hash_type);
    if (hash_type == HASH_TYPE_NONE) {
        // No filter applied, just clone all
        __u32 tap = skb->ifindex;
        __u32 *mirror_ifindex = bpf_map_lookup_elem(&mirror_map, &tap);
        if (mirror_ifindex)
            bpf_clone_redirect(skb, *mirror_ifindex, 0);

    } else {
        // Clone base on filter, apply only on IP packet
        struct ethhdr *eth = data;
        if ((void *)(eth + 1) > data_end)
            return TC_ACT_OK;

        void *trans_data;
        struct in6_addr sip6, dip6;
        __be32 sip = 0, dip = 0;
        __be16 sport = 0, dport = 0;
        __u8 proto = 0;
        __u8 ip_version = 4;

        if (eth->h_proto == bpf_htons(ETH_P_IP)) {
            struct iphdr *iph = data + sizeof(*eth);
            if ((void*)(iph + 1) > data_end)
                return TC_ACT_OK;

            proto = iph->protocol;
            trans_data = (void*)iph + (iph->ihl * 4);
            sip = iph->saddr;
            dip = iph->daddr;
            ip_version = 4;

        } else if (eth->h_proto == bpf_htons(ETH_P_IPV6)) {
            struct ipv6hdr *ip6h = data + sizeof(*eth);
            if ((void *)(ip6h + 1) > data_end)
                return TC_ACT_OK;

            proto = ip6h->nexthdr;
            trans_data = ip6h + 1;
            sip6 = ip6h->saddr;
            dip6 = ip6h->daddr;
            ip_version = 6;
        } else {
            // unknown packet
            return TC_ACT_OK;
        }

        if (proto == IPPROTO_TCP) {
            struct tcphdr *tcph = trans_data;

            if ((void *)(trans_data + sizeof(*tcph)) > data_end)
                return TC_ACT_OK;

            sport = tcph->source;
            dport = tcph->dest;
        } else {
            struct udphdr *udph = trans_data;

            if ((void *)(trans_data + sizeof(*udph)) > data_end)
                return TC_ACT_OK;

            sport = udph->source;
            dport = udph->dest;
        }

        bpf_printk("IP: saddr=%u, daddr=%u, proto=%u",
            bpf_ntohl(sip), bpf_ntohl(dip), proto);
        bpf_printk("IP: sport=%u, dport=%u",
            bpf_ntohs(sport), bpf_ntohs(dport));
        struct flow_key pkt_key = {
            .ip_version = ip_version
        };

        if (hash_type & HASH_TYPE_SRC)
            if (ip_version == 4) {
                pkt_key.saddr = sip;
            } else {
                __builtin_memcpy(pkt_key.saddr6, sip6.in6_u.u6_addr8, 16);
            }
        if (hash_type & HASH_TYPE_DST)
            if (ip_version == 4) {
                pkt_key.daddr = dip;
            } else {
                __builtin_memcpy(pkt_key.daddr6, sip6.in6_u.u6_addr8, 16);
            }
        if (hash_type & HASH_TYPE_PROTO)
            pkt_key.proto = proto;
        if (hash_type & HASH_TYPE_SPORT)
            pkt_key.sport = sport;
        if (hash_type & HASH_TYPE_DPORT)
            pkt_key.dport = dport;

        bpf_printk("SPORT flag = %u", hash_type & HASH_TYPE_SPORT);
        bpf_printk("KEY: saddr=%lu, daddr=%lu, proto=%u",
            pkt_key.saddr, pkt_key.daddr, pkt_key.proto);
        bpf_printk("KEY: sport=%u, dport=%u",
            pkt_key.sport, pkt_key.dport);
        __u8 *action = bpf_map_lookup_elem(&filter_map, &pkt_key);
        if (action && *action == MIRROR) {
            __u32 tap = skb->ifindex;
            __u32 *mirror_ifindex = bpf_map_lookup_elem(&mirror_map, &tap);
            if (mirror_ifindex) {
                bpf_clone_redirect(skb, *mirror_ifindex, 0);
            } else {
                log_err(0, "No mirror port for this pkt");
            }
        } else {
            log_err(0, "No mirror action for this pkt");
        }
    }

    return TC_ACT_OK;
}
