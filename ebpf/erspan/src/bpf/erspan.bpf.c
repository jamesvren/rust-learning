#include "vmlinux.h"
#include <bpf/bpf_helpers.h>
#include <bpf/bpf_endian.h>

//#include <linux/pkt_cls.h>
#define TC_ACT_UNSPEC    (-1)
#define TC_ACT_OK        0
#define TC_ACT_SHOT      2

#define ERSPAN_V1 1

char _license[] SEC("license") = "GPL";

#define log_err(__ret) bpf_printk("ERROR line:%d ret:%d\n", __LINE__, __ret)

struct {
    __uint(type, BPF_MAP_TYPE_ARRAY);
    __uint(max_entries, 10);
    __type(value, u16);
    __type(key, u32);
} ports SEC(".maps");

SEC("tc")
int erspan_get_tunnel(struct __sk_buff *skb)
{
    struct bpf_tunnel_key key;
    struct erspan_metadata md;
    int ret;
    __u32 index = 0;

    ret = bpf_skb_get_tunnel_key(skb, &key, sizeof(key), 0);
    if (ret < 0) {
	log_err(ret);
	return TC_ACT_SHOT;
    }

    ret = bpf_skb_get_tunnel_opt(skb, &md, sizeof(md));
    if (ret < 0) {
	log_err(ret);
	return TC_ACT_SHOT;
    }

    bpf_printk("key %d remote ip 0x%x erspan version %d\n",
	    key.tunnel_id, key.remote_ipv4, md.version);

#ifdef ERSPAN_V1
    index = bpf_ntohl(md.u.index);
    bpf_printk("\t index %x\n", index);
#else
    bpf_printk("\tdirection %d hwid %x timestamp %u\n",
	    md.u.md2.dir,
	    (md.u.md2.hwid_upper << 4) + md.u.md2.hwid,
	    bpf_ntohl(md.u.md2.timestamp));
#endif

    return TC_ACT_OK;
}
