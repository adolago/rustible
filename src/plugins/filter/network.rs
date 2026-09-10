//! IP address filters for Jinja2 templates.
//!
//! Implements the subset of Ansible's `ipaddr` filter family that playbooks
//! rely on most: validation, network math, and address selection for IPv4 and
//! IPv6.
//!
//! # Available Filters
//!
//! - `ipaddr`: Validate/filter addresses and answer queries about them
//! - `ipv4` / `ipv6`: Keep only addresses of one family
//! - `ipsubnet`: Split a network into subnets or report an address' subnet
//! - `ipmath`: Add or subtract an offset from an address
//! - `nthhost`: Select the nth address inside a network
//! - `network_in_usable`: Test whether an address falls in a network's usable range
//! - `cidr_merge`: Merge a list of addresses into the smallest CIDR list
//! - `ipwrap`: Wrap IPv6 addresses in brackets
//!
//! # Examples
//!
//! ```jinja2
//! {{ '192.168.0.1/24' | ipaddr('network') }}
//! {{ ['10.0.0.1', 'nonsense'] | ipaddr }}
//! {{ '10.0.0.0/8' | nthhost(305) }}
//! {{ '192.168.0.0/16' | ipsubnet(20, 0) }}
//! ```
//!
//! # Ansible Compatibility
//!
//! Invalid input yields `false` (as in Ansible) rather than an error, so
//! `ipaddr` stays usable as a `when:` predicate. Queries not implemented here
//! raise an error instead of silently returning a wrong answer.

use minijinja::value::ValueKind;
use minijinja::{Environment, Error, ErrorKind, Value};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

/// Register all network filters with the given environment.
pub fn register_filters(env: &mut Environment<'static>) {
    env.add_filter("ipaddr", ipaddr);
    env.add_filter("ipv4", ipv4);
    env.add_filter("ipv6", ipv6);
    env.add_filter("ipsubnet", ipsubnet);
    env.add_filter("ipmath", ipmath);
    env.add_filter("nthhost", nthhost);
    env.add_filter("network_in_usable", network_in_usable);
    env.add_filter("cidr_merge", cidr_merge);
    env.add_filter("ipwrap", ipwrap);
}

/// An address with an optional prefix length, mirroring Ansible's handling of
/// bare addresses and CIDR notation in the same filter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct IpNet {
    addr: IpAddr,
    prefix: u8,
    /// Whether the source text carried an explicit prefix.
    explicit_prefix: bool,
}

impl IpNet {
    fn parse(text: &str) -> Option<Self> {
        let text = text.trim();
        match text.split_once('/') {
            None => {
                let addr: IpAddr = text.parse().ok()?;
                Some(Self {
                    prefix: host_prefix(&addr),
                    addr,
                    explicit_prefix: false,
                })
            }
            Some((addr, prefix)) => {
                let addr: IpAddr = addr.trim().parse().ok()?;
                let max = host_prefix(&addr);
                let prefix = match prefix.trim().parse::<u8>() {
                    Ok(prefix) => prefix,
                    // Ansible accepts a dotted netmask (192.0.2.1/255.255.255.0).
                    Err(_) => netmask_to_prefix(prefix.trim(), &addr)?,
                };
                if prefix > max {
                    return None;
                }
                Some(Self {
                    addr,
                    prefix,
                    explicit_prefix: true,
                })
            }
        }
    }

    fn is_ipv4(&self) -> bool {
        self.addr.is_ipv4()
    }

    fn bits(&self) -> u32 {
        host_prefix(&self.addr) as u32
    }

    fn as_u128(&self) -> u128 {
        match self.addr {
            IpAddr::V4(v4) => u32::from(v4) as u128,
            IpAddr::V6(v6) => u128::from(v6),
        }
    }

    fn with_value(&self, value: u128) -> IpNet {
        IpNet {
            addr: from_u128(value, self.is_ipv4()),
            prefix: self.prefix,
            explicit_prefix: self.explicit_prefix,
        }
    }

    /// Number of addresses the prefix covers.
    fn size(&self) -> u128 {
        let host_bits = self.bits() - self.prefix as u32;
        if host_bits >= 128 {
            u128::MAX
        } else {
            1u128 << host_bits
        }
    }

    fn network(&self) -> u128 {
        let host_bits = self.bits() - self.prefix as u32;
        if host_bits >= 128 {
            0
        } else {
            (self.as_u128() >> host_bits) << host_bits
        }
    }

    fn broadcast(&self) -> u128 {
        self.network() + (self.size() - 1)
    }

    fn netmask(&self) -> u128 {
        let bits = self.bits();
        let host_bits = bits - self.prefix as u32;
        let full = if bits == 32 {
            u32::MAX as u128
        } else {
            u128::MAX
        };
        if host_bits >= bits {
            0
        } else {
            (full >> host_bits) << host_bits
        }
    }

    /// Render as Ansible does: keep the prefix when the input carried one.
    fn render(&self) -> String {
        if self.explicit_prefix {
            format!("{}/{}", self.addr, self.prefix)
        } else {
            self.addr.to_string()
        }
    }

    fn render_with_prefix(&self) -> String {
        format!("{}/{}", self.addr, self.prefix)
    }

    fn is_private(&self) -> bool {
        match self.addr {
            IpAddr::V4(v4) => v4.is_private(),
            // fc00::/7 is the IPv6 unique local range.
            IpAddr::V6(v6) => (v6.segments()[0] & 0xfe00) == 0xfc00,
        }
    }

    fn is_link_local(&self) -> bool {
        match self.addr {
            IpAddr::V4(v4) => v4.is_link_local(),
            IpAddr::V6(v6) => (v6.segments()[0] & 0xffc0) == 0xfe80,
        }
    }

    fn is_public(&self) -> bool {
        !(self.is_private()
            || self.is_link_local()
            || self.addr.is_loopback()
            || self.addr.is_multicast()
            || self.addr.is_unspecified()
            || self.is_reserved())
    }

    fn is_reserved(&self) -> bool {
        match self.addr {
            // 240.0.0.0/4 plus the documentation and benchmark ranges are not
            // routable on the public internet.
            IpAddr::V4(v4) => {
                let octets = v4.octets();
                octets[0] >= 240
                    || (octets[0] == 192 && octets[1] == 0 && octets[2] == 2)
                    || (octets[0] == 198 && (octets[1] == 18 || octets[1] == 19))
                    || (octets[0] == 198 && octets[1] == 51 && octets[2] == 100)
                    || (octets[0] == 203 && octets[1] == 0 && octets[2] == 113)
                    || (octets[0] == 100 && (64..128).contains(&octets[1]))
            }
            IpAddr::V6(v6) => {
                let segments = v6.segments();
                // 2001:db8::/32 documentation range.
                segments[0] == 0x2001 && segments[1] == 0x0db8
            }
        }
    }
}

fn host_prefix(addr: &IpAddr) -> u8 {
    if addr.is_ipv4() {
        32
    } else {
        128
    }
}

fn from_u128(value: u128, ipv4: bool) -> IpAddr {
    if ipv4 {
        IpAddr::V4(Ipv4Addr::from(value as u32))
    } else {
        IpAddr::V6(Ipv6Addr::from(value))
    }
}

fn netmask_to_prefix(mask: &str, addr: &IpAddr) -> Option<u8> {
    let mask: Ipv4Addr = mask.parse().ok()?;
    if !addr.is_ipv4() {
        return None;
    }
    let bits = u32::from(mask);
    // A netmask must be a run of ones followed by a run of zeroes.
    let ones = bits.leading_ones();
    if bits.count_ones() != ones {
        return None;
    }
    Some(ones as u8)
}

/// Read a filter input as one address, or as the list of addresses it holds.
fn as_inputs(value: &Value) -> Vec<String> {
    match value.kind() {
        ValueKind::Seq | ValueKind::Iterable => value
            .try_iter()
            .map(|iter| iter.map(|item| item.to_string()).collect())
            .unwrap_or_default(),
        _ => vec![value.to_string()],
    }
}

fn is_sequence(value: &Value) -> bool {
    matches!(value.kind(), ValueKind::Seq | ValueKind::Iterable)
}

/// Validate addresses and answer queries about them.
///
/// # Ansible Compatibility
///
/// Supports the query strings Ansible documents for single addresses plus an
/// integer index into a network. Unknown queries are an error.
fn ipaddr(value: Value, query: Option<Value>) -> Result<Value, Error> {
    let query = query.map(|q| q.to_string()).unwrap_or_default();
    apply_per_item(&value, |net| ipaddr_one(net, &query))
}

/// Apply `op` to every address, dropping the ones it rejects.
///
/// A scalar input yields a scalar result (or `false`); a list input yields the
/// filtered list, which is how Ansible's ipaddr filters behave.
fn apply_per_item<F>(value: &Value, op: F) -> Result<Value, Error>
where
    F: Fn(&IpNet) -> Result<Option<Value>, Error>,
{
    let sequence = is_sequence(value);
    let mut results = Vec::new();
    for text in as_inputs(value) {
        let Some(net) = IpNet::parse(&text) else {
            continue;
        };
        if let Some(result) = op(&net)? {
            results.push(result);
        }
    }

    if sequence {
        Ok(Value::from(results))
    } else {
        Ok(results.into_iter().next().unwrap_or(Value::from(false)))
    }
}

fn ipaddr_one(net: &IpNet, query: &str) -> Result<Option<Value>, Error> {
    // An integer query selects the nth address inside the network.
    if let Ok(index) = query.parse::<i128>() {
        return Ok(nth_address(net, index).map(|net| Value::from(net.render_with_prefix())));
    }

    let result = match query {
        "" => Value::from(net.render()),
        "address" | "ip" => Value::from(net.addr.to_string()),
        "address/prefix" | "host/prefix" | "host" => Value::from(net.render_with_prefix()),
        "prefix" => Value::from(net.prefix),
        "netmask" => Value::from(from_u128(net.netmask(), net.is_ipv4()).to_string()),
        "hostmask" | "wildcard" => {
            let bits = if net.is_ipv4() {
                u32::MAX as u128
            } else {
                u128::MAX
            };
            Value::from(from_u128(bits ^ net.netmask(), net.is_ipv4()).to_string())
        }
        "network" => Value::from(from_u128(net.network(), net.is_ipv4()).to_string()),
        "broadcast" => {
            // A single-address prefix has no broadcast address.
            if net.prefix as u32 >= net.bits() - 1 {
                return Ok(None);
            }
            Value::from(from_u128(net.broadcast(), net.is_ipv4()).to_string())
        }
        "net" | "subnet" | "network/prefix" => Value::from(format!(
            "{}/{}",
            from_u128(net.network(), net.is_ipv4()),
            net.prefix
        )),
        "size" => Value::from(net.size()),
        "first_usable" => match first_usable(net) {
            Some(value) => Value::from(from_u128(value, net.is_ipv4()).to_string()),
            None => return Ok(None),
        },
        "last_usable" => match last_usable(net) {
            Some(value) => Value::from(from_u128(value, net.is_ipv4()).to_string()),
            None => return Ok(None),
        },
        "version" => Value::from(if net.is_ipv4() { 4 } else { 6 }),
        "4" | "ipv4" => {
            if !net.is_ipv4() {
                return Ok(None);
            }
            Value::from(net.render())
        }
        "6" | "ipv6" => {
            if net.is_ipv4() {
                return Ok(None);
            }
            Value::from(net.render())
        }
        "public" => return Ok(net.is_public().then(|| Value::from(net.render()))),
        "private" => return Ok(net.is_private().then(|| Value::from(net.render()))),
        "loopback" | "lo" => return Ok(net.addr.is_loopback().then(|| Value::from(net.render()))),
        "multicast" => return Ok(net.addr.is_multicast().then(|| Value::from(net.render()))),
        "link-local" => return Ok(net.is_link_local().then(|| Value::from(net.render()))),
        "unspecified" => return Ok(net.addr.is_unspecified().then(|| Value::from(net.render()))),
        other => {
            return Err(Error::new(
                ErrorKind::InvalidOperation,
                format!("ipaddr: unsupported query '{}'", other),
            ))
        }
    };
    Ok(Some(result))
}

/// The nth address inside a network, counting negatives from the end.
fn nth_address(net: &IpNet, index: i128) -> Option<IpNet> {
    let size = net.size();
    let offset = if index < 0 {
        let back = index.unsigned_abs();
        if back > size {
            return None;
        }
        size - back
    } else {
        let forward = index as u128;
        if forward >= size {
            return None;
        }
        forward
    };
    Some(net.with_value(net.network() + offset))
}

/// First address usable by a host, excluding the network address on IPv4
/// networks that have room for one.
fn first_usable(net: &IpNet) -> Option<u128> {
    if net.is_ipv4() && net.prefix < 31 {
        Some(net.network() + 1)
    } else {
        Some(net.network())
    }
}

/// Last address usable by a host, excluding the IPv4 broadcast address.
fn last_usable(net: &IpNet) -> Option<u128> {
    if net.is_ipv4() && net.prefix < 31 {
        Some(net.broadcast() - 1)
    } else {
        Some(net.broadcast())
    }
}

/// Keep only IPv4 addresses.
fn ipv4(value: Value, query: Option<Value>) -> Result<Value, Error> {
    let query = query.map(|q| q.to_string()).unwrap_or_default();
    apply_per_item(&value, |net| {
        if !net.is_ipv4() {
            return Ok(None);
        }
        ipaddr_one(net, &query)
    })
}

/// Keep only IPv6 addresses.
fn ipv6(value: Value, query: Option<Value>) -> Result<Value, Error> {
    let query = query.map(|q| q.to_string()).unwrap_or_default();
    apply_per_item(&value, |net| {
        if net.is_ipv4() {
            return Ok(None);
        }
        ipaddr_one(net, &query)
    })
}

/// Report or split subnets.
///
/// With no argument the address is reported as a host subnet. With a prefix
/// length the number of subnets of that size is returned, and with an
/// additional index the subnet at that position.
fn ipsubnet(value: Value, prefix: Option<u8>, index: Option<i128>) -> Result<Value, Error> {
    let text = value.to_string();
    let Some(net) = IpNet::parse(&text) else {
        return Ok(Value::from(false));
    };

    let Some(prefix) = prefix else {
        return Ok(Value::from(format!("{}/{}", net.addr, net.prefix)));
    };

    if prefix as u32 > net.bits() || prefix < net.prefix {
        return Err(Error::new(
            ErrorKind::InvalidOperation,
            format!("ipsubnet: prefix /{} is not inside /{}", prefix, net.prefix),
        ));
    }

    let count = 1u128 << (prefix as u32 - net.prefix as u32);
    let Some(index) = index else {
        return Ok(Value::from(count));
    };

    let offset = if index < 0 {
        let back = index.unsigned_abs();
        if back > count {
            return Ok(Value::from(false));
        }
        count - back
    } else {
        let forward = index as u128;
        if forward >= count {
            return Ok(Value::from(false));
        }
        forward
    };

    let step = 1u128 << (net.bits() - prefix as u32);
    let base = net.network() + offset * step;
    Ok(Value::from(format!(
        "{}/{}",
        from_u128(base, net.is_ipv4()),
        prefix
    )))
}

/// Add (or subtract) an offset from an address.
fn ipmath(value: Value, offset: i128) -> Result<Value, Error> {
    let text = value.to_string();
    let Some(net) = IpNet::parse(&text) else {
        return Err(Error::new(
            ErrorKind::InvalidOperation,
            format!("ipmath: '{}' is not a valid IP address", text),
        ));
    };

    let current = net.as_u128();
    let result = if offset < 0 {
        current.checked_sub(offset.unsigned_abs())
    } else {
        current.checked_add(offset as u128)
    };

    let max = if net.is_ipv4() {
        u32::MAX as u128
    } else {
        u128::MAX
    };
    match result {
        Some(result) if result <= max => {
            Ok(Value::from(from_u128(result, net.is_ipv4()).to_string()))
        }
        _ => Err(Error::new(
            ErrorKind::InvalidOperation,
            format!(
                "ipmath: offset {} moves '{}' outside its address family",
                offset, text
            ),
        )),
    }
}

/// Select the nth address inside a network, without the prefix.
fn nthhost(value: Value, index: i128) -> Result<Value, Error> {
    let text = value.to_string();
    let Some(net) = IpNet::parse(&text) else {
        return Ok(Value::from(false));
    };
    Ok(match nth_address(&net, index) {
        Some(net) => Value::from(net.addr.to_string()),
        None => Value::from(false),
    })
}

/// Test whether an address falls inside a network's usable range.
fn network_in_usable(value: Value, other: Value) -> Result<Value, Error> {
    let network = value.to_string();
    let candidate = other.to_string();
    let (Some(network), Some(candidate)) = (IpNet::parse(&network), IpNet::parse(&candidate))
    else {
        return Ok(Value::from(false));
    };
    if network.is_ipv4() != candidate.is_ipv4() {
        return Ok(Value::from(false));
    }

    let (Some(first), Some(last)) = (first_usable(&network), last_usable(&network)) else {
        return Ok(Value::from(false));
    };
    let address = candidate.as_u128();
    Ok(Value::from(address >= first && address <= last))
}

/// Merge addresses into the smallest list of CIDR ranges that covers them.
fn cidr_merge(value: Value, action: Option<String>) -> Result<Value, Error> {
    let action = action.unwrap_or_else(|| "merge".to_string());
    if action != "merge" && action != "span" {
        return Err(Error::new(
            ErrorKind::InvalidOperation,
            format!(
                "cidr_merge: unsupported action '{}' (supported: merge, span)",
                action
            ),
        ));
    }

    let mut nets: Vec<IpNet> = as_inputs(&value)
        .iter()
        .filter_map(|text| IpNet::parse(text))
        .collect();
    if nets.is_empty() {
        return Ok(Value::from(Vec::<Value>::new()));
    }
    if nets.iter().any(|net| net.is_ipv4() != nets[0].is_ipv4()) {
        return Err(Error::new(
            ErrorKind::InvalidOperation,
            "cidr_merge: cannot mix IPv4 and IPv6 addresses",
        ));
    }

    nets.sort_by_key(|net| (net.network(), net.prefix));

    if action == "span" {
        let ipv4 = nets[0].is_ipv4();
        let low = nets.iter().map(|net| net.network()).min().unwrap_or(0);
        let high = nets.iter().map(|net| net.broadcast()).max().unwrap_or(0);
        let bits = if ipv4 { 32 } else { 128 };
        // The span is the shortest prefix that contains both ends.
        let mut prefix = bits;
        while prefix > 0 {
            let host_bits = bits - prefix;
            let mask = if host_bits >= 128 {
                0
            } else {
                (u128::MAX >> host_bits) << host_bits
            };
            if low & mask == high & mask {
                break;
            }
            prefix -= 1;
        }
        let host_bits = bits - prefix;
        let base = if host_bits >= 128 {
            0
        } else {
            (low >> host_bits) << host_bits
        };
        return Ok(Value::from(format!("{}/{}", from_u128(base, ipv4), prefix)));
    }

    // Absorb ranges contained in an earlier one, then coalesce adjacent pairs.
    let mut merged: Vec<IpNet> = Vec::new();
    for net in nets {
        match merged.last() {
            Some(last) if net.broadcast() <= last.broadcast() => continue,
            _ => merged.push(net),
        }
    }

    let mut changed = true;
    while changed {
        changed = false;
        let mut index = 0;
        while index + 1 < merged.len() {
            let (left, right) = (merged[index], merged[index + 1]);
            let siblings = left.prefix == right.prefix
                && left.prefix > 0
                && left.broadcast() + 1 == right.network()
                && (left.network() >> (left.bits() - left.prefix as u32 + 1))
                    == (right.network() >> (right.bits() - right.prefix as u32 + 1));
            if siblings {
                let combined = IpNet {
                    addr: from_u128(left.network(), left.is_ipv4()),
                    prefix: left.prefix - 1,
                    explicit_prefix: true,
                };
                merged[index] = combined;
                merged.remove(index + 1);
                changed = true;
            } else {
                index += 1;
            }
        }
    }

    Ok(Value::from(
        merged
            .into_iter()
            .map(|net| Value::from(net.render_with_prefix()))
            .collect::<Vec<_>>(),
    ))
}

/// Wrap IPv6 addresses in brackets, leaving IPv4 addresses untouched.
fn ipwrap(value: Value, query: Option<Value>) -> Result<Value, Error> {
    let query = query.map(|q| q.to_string()).unwrap_or_default();
    apply_per_item(&value, |net| {
        let Some(result) = ipaddr_one(net, &query)? else {
            return Ok(None);
        };
        if net.is_ipv4() {
            return Ok(Some(result));
        }
        Ok(Some(Value::from(format!("[{}]", result))))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn render(template: &str) -> String {
        let mut env = Environment::new();
        register_filters(&mut env);
        env.template_from_str(template)
            .unwrap()
            .render(Value::UNDEFINED)
            .unwrap()
    }

    #[test]
    fn test_ipaddr_validates() {
        assert_eq!(render("{{ '192.168.0.1' | ipaddr }}"), "192.168.0.1");
        assert_eq!(render("{{ 'not-an-ip' | ipaddr }}"), "false");
        assert_eq!(render("{{ '::1' | ipaddr }}"), "::1");
    }

    #[test]
    fn test_ipaddr_filters_lists() {
        assert_eq!(
            render("{{ ['10.0.0.1', 'bogus', '::1'] | ipaddr | join(',') }}"),
            "10.0.0.1,::1"
        );
        assert_eq!(
            render("{{ ['10.0.0.1', '::1'] | ipv4 | join(',') }}"),
            "10.0.0.1"
        );
        assert_eq!(
            render("{{ ['10.0.0.1', '::1'] | ipv6 | join(',') }}"),
            "::1"
        );
    }

    #[test]
    fn test_ipaddr_queries() {
        assert_eq!(
            render("{{ '192.168.0.5/24' | ipaddr('address') }}"),
            "192.168.0.5"
        );
        assert_eq!(
            render("{{ '192.168.0.5/24' | ipaddr('netmask') }}"),
            "255.255.255.0"
        );
        assert_eq!(
            render("{{ '192.168.0.5/24' | ipaddr('network') }}"),
            "192.168.0.0"
        );
        assert_eq!(
            render("{{ '192.168.0.5/24' | ipaddr('broadcast') }}"),
            "192.168.0.255"
        );
        assert_eq!(render("{{ '192.168.0.5/24' | ipaddr('prefix') }}"), "24");
        assert_eq!(render("{{ '192.168.0.5/24' | ipaddr('size') }}"), "256");
        assert_eq!(
            render("{{ '192.168.0.5/24' | ipaddr('net') }}"),
            "192.168.0.0/24"
        );
        assert_eq!(
            render("{{ '192.168.0.5/24' | ipaddr('hostmask') }}"),
            "0.0.0.255"
        );
        assert_eq!(
            render("{{ '192.168.0.5/24' | ipaddr('first_usable') }}"),
            "192.168.0.1"
        );
        assert_eq!(
            render("{{ '192.168.0.5/24' | ipaddr('last_usable') }}"),
            "192.168.0.254"
        );
    }

    #[test]
    fn test_ipaddr_dotted_netmask_input() {
        assert_eq!(
            render("{{ '192.168.0.5/255.255.255.0' | ipaddr('net') }}"),
            "192.168.0.0/24"
        );
        assert_eq!(
            render("{{ '192.168.0.5/255.0.255.0' | ipaddr }}"),
            "false",
            "a non-contiguous netmask is not a valid prefix"
        );
    }

    #[test]
    fn test_ipaddr_classification() {
        assert_eq!(render("{{ '10.1.2.3' | ipaddr('private') }}"), "10.1.2.3");
        assert_eq!(render("{{ '8.8.8.8' | ipaddr('private') }}"), "false");
        assert_eq!(render("{{ '8.8.8.8' | ipaddr('public') }}"), "8.8.8.8");
        assert_eq!(render("{{ '127.0.0.1' | ipaddr('public') }}"), "false");
        assert_eq!(
            render("{{ '169.254.1.1' | ipaddr('link-local') }}"),
            "169.254.1.1"
        );
        assert_eq!(render("{{ 'fe80::1' | ipaddr('link-local') }}"), "fe80::1");
        assert_eq!(render("{{ 'fd00::1' | ipaddr('private') }}"), "fd00::1");
    }

    #[test]
    fn test_ipaddr_index() {
        assert_eq!(
            render("{{ '192.168.0.0/24' | ipaddr(1) }}"),
            "192.168.0.1/24"
        );
        assert_eq!(
            render("{{ '192.168.0.0/24' | ipaddr(-1) }}"),
            "192.168.0.255/24"
        );
        assert_eq!(render("{{ '192.168.0.0/24' | ipaddr(256) }}"), "false");
    }

    #[test]
    fn test_nthhost() {
        assert_eq!(render("{{ '10.0.0.0/8' | nthhost(305) }}"), "10.0.1.49");
        assert_eq!(render("{{ '10.0.0.0/8' | nthhost(0) }}"), "10.0.0.0");
        assert_eq!(render("{{ 'bogus' | nthhost(1) }}"), "false");
    }

    #[test]
    fn test_ipmath() {
        assert_eq!(render("{{ '192.168.1.5' | ipmath(5) }}"), "192.168.1.10");
        assert_eq!(render("{{ '192.168.1.5' | ipmath(-5) }}"), "192.168.1.0");
        assert_eq!(render("{{ '192.168.1.5' | ipmath(250) }}"), "192.168.1.255");
        assert_eq!(render("{{ '2001:db8::1' | ipmath(1) }}"), "2001:db8::2");
    }

    #[test]
    fn test_ipsubnet() {
        assert_eq!(
            render("{{ '192.168.144.5' | ipsubnet }}"),
            "192.168.144.5/32"
        );
        assert_eq!(render("{{ '192.168.0.0/16' | ipsubnet(20) }}"), "16");
        assert_eq!(
            render("{{ '192.168.0.0/16' | ipsubnet(20, 0) }}"),
            "192.168.0.0/20"
        );
        assert_eq!(
            render("{{ '192.168.0.0/16' | ipsubnet(20, 1) }}"),
            "192.168.16.0/20"
        );
        assert_eq!(
            render("{{ '192.168.0.0/16' | ipsubnet(20, -1) }}"),
            "192.168.240.0/20"
        );
    }

    #[test]
    fn test_network_in_usable() {
        assert_eq!(
            render("{{ '192.168.0.0/24' | network_in_usable('192.168.0.1') }}"),
            "true"
        );
        assert_eq!(
            render("{{ '192.168.0.0/24' | network_in_usable('192.168.0.255') }}"),
            "false"
        );
        assert_eq!(
            render("{{ '192.168.0.0/24' | network_in_usable('192.168.1.1') }}"),
            "false"
        );
    }

    #[test]
    fn test_cidr_merge() {
        assert_eq!(
            render("{{ ['192.168.0.0/24', '192.168.1.0/24'] | cidr_merge | join(',') }}"),
            "192.168.0.0/23"
        );
        assert_eq!(
            render("{{ ['192.168.0.0/24', '192.168.4.0/24'] | cidr_merge | join(',') }}"),
            "192.168.0.0/24,192.168.4.0/24"
        );
        assert_eq!(
            render("{{ ['192.168.0.0/24', '192.168.4.0/24'] | cidr_merge('span') }}"),
            "192.168.0.0/21"
        );
    }

    #[test]
    fn test_ipwrap() {
        assert_eq!(render("{{ '::1' | ipwrap }}"), "[::1]");
        assert_eq!(render("{{ '10.0.0.1' | ipwrap }}"), "10.0.0.1");
    }

    #[test]
    fn test_unknown_query_is_an_error() {
        let mut env = Environment::new();
        register_filters(&mut env);
        let tmpl = env
            .template_from_str("{{ '10.0.0.1' | ipaddr('bogus-query') }}")
            .unwrap();
        let err = tmpl.render(Value::UNDEFINED).unwrap_err();
        assert!(err.to_string().contains("unsupported query"));
    }
}
