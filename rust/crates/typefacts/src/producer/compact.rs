//! Expanding a compact demand snapshot (Go: `CompactDemandsV3.Expand`,
//! `protocolv3_compact.go`). The client packs with [`crate::v3::compact_demands`];
//! the producer reads it back into plain demands.

use crate::Location;
use crate::v3::{
    CompactDemandGroup, CompactDemands, DEMAND_FLAG_ARRAY_SHAPE, DEMAND_FLAG_ASYNC,
    DEMAND_FLAG_CALL_RESULT_DOMAIN, DEMAND_FLAG_CALLABILITY, DEMAND_FLAG_CONSTANT_VALUE,
    DEMAND_FLAG_CONSTRUCTABILITY, DEMAND_FLAG_DECLARED_KEYS, DEMAND_FLAG_LIBRARY_TYPES,
    DEMAND_FLAG_PARAMETER_OBJECT_SHAPE, DEMAND_FLAG_PRIMITIVE_LITERAL_CANDIDATES,
    DEMAND_FLAG_PRIMITIVE_VALUE_DOMAIN, DEMAND_FLAG_REFERENCE_SPACE, DEMAND_FLAG_REFERENCES,
    DEMAND_FLAG_RESOLVED_CALL, DEMAND_FLAG_RUNTIME_IDENTITY, DEMAND_FLAG_RUNTIME_VALUE_DOMAIN,
    DEMAND_FLAG_STRUCTURAL_ACCESSOR, DEMAND_FLAG_SYMBOL, DEMAND_FLAG_TUPLE_SHAPE,
    DEMAND_FLAG_TYPE_DESCRIPTOR, EntityDemand,
};

/// Reads an unsigned LEB128 value and the bytes after it.
fn take_uvarint(input: &[u8]) -> Result<(u64, &[u8]), String> {
    let mut value = 0_u64;
    for (index, byte) in input.iter().enumerate() {
        if index == 10 {
            break;
        }
        if index == 9 && *byte > 1 {
            return Err("overflowing compact demand varint".to_owned());
        }
        value |= u64::from(byte & 0x7f) << (7 * index);
        if byte & 0x80 == 0 {
            return Ok((value, &input[index + 1..]));
        }
    }
    Err(if input.len() >= 10 {
        "overflowing compact demand varint".to_owned()
    } else {
        "truncated compact demand varint".to_owned()
    })
}

fn lookup(strings: &[String], index: u64) -> Result<&str, String> {
    usize::try_from(index)
        .ok()
        .and_then(|index| strings.get(index))
        .map(String::as_str)
        .ok_or_else(|| {
            format!(
                "compact string index {index} out of range ({} strings)",
                strings.len()
            )
        })
}

/// The number of demand rows and query locations a packed group holds, after
/// checking that every row parses (Go: `CompactDemandsV3.demandShape`).
pub fn compact_demand_shape(groups: &[CompactDemandGroup]) -> Result<(usize, usize), String> {
    let (mut demands, mut queries) = (0, 0);
    for group in groups {
        let mut packed = group.1.as_slice();
        while !packed.is_empty() {
            let (header, mut rest) = take_uvarint(packed)?;
            for _ in 0..2 {
                rest = take_uvarint(rest)?.1;
            }
            if header & 1 != 0 {
                queries += 1;
                for _ in 0..3 {
                    rest = take_uvarint(rest)?.1;
                }
            }
            packed = rest;
            demands += 1;
        }
    }
    Ok((demands, queries))
}

fn plain_demand(location: Location, flags: u64) -> EntityDemand {
    let has = |flag: u64| flags & flag != 0;
    EntityDemand {
        location,
        symbol: has(DEMAND_FLAG_SYMBOL),
        references: has(DEMAND_FLAG_REFERENCES),
        type_descriptor: has(DEMAND_FLAG_TYPE_DESCRIPTOR),
        resolved_call: has(DEMAND_FLAG_RESOLVED_CALL),
        r#async: has(DEMAND_FLAG_ASYNC),
        structural_accessor: has(DEMAND_FLAG_STRUCTURAL_ACCESSOR),
        callability: has(DEMAND_FLAG_CALLABILITY),
        reference_space: has(DEMAND_FLAG_REFERENCE_SPACE),
        runtime_identity: has(DEMAND_FLAG_RUNTIME_IDENTITY),
        runtime_value_domain: has(DEMAND_FLAG_RUNTIME_VALUE_DOMAIN),
        call_result_domain: has(DEMAND_FLAG_CALL_RESULT_DOMAIN),
        constant_value: has(DEMAND_FLAG_CONSTANT_VALUE),
        array_shape: has(DEMAND_FLAG_ARRAY_SHAPE),
        tuple_shape: has(DEMAND_FLAG_TUPLE_SHAPE),
        library_types: has(DEMAND_FLAG_LIBRARY_TYPES),
        primitive_value_domain: has(DEMAND_FLAG_PRIMITIVE_VALUE_DOMAIN),
        primitive_literal_candidates: has(DEMAND_FLAG_PRIMITIVE_LITERAL_CANDIDATES),
        parameter_object_shape: has(DEMAND_FLAG_PARAMETER_OBJECT_SHAPE),
        constructability: has(DEMAND_FLAG_CONSTRUCTABILITY),
        declared_keys: has(DEMAND_FLAG_DECLARED_KEYS),
        query_location: None,
    }
}

/// Visits one packed run, handing every row's flags and locations to `visit`
/// (`None` query when the row has none). Shared by the expansion paths.
fn visit_group(
    group: &CompactDemandGroup,
    strings: &[String],
    mut visit: impl FnMut(u64, u64, u64, Option<(u64, u64, u64)>) -> Result<(), String>,
) -> Result<(), String> {
    let _ = lookup(strings, group.0)?;
    let mut packed = group.1.as_slice();
    let mut previous_start = 0_u64;
    while !packed.is_empty() {
        let (header, rest) = take_uvarint(packed)?;
        let (start_delta, rest) = take_uvarint(rest)?;
        let (length, rest) = take_uvarint(rest)?;
        let start = previous_start
            .checked_add(start_delta)
            .ok_or("compact demand location overflow")?;
        start
            .checked_add(length)
            .ok_or("compact demand location overflow")?;
        previous_start = start;
        packed = rest;
        let mut query = None;
        if header & 1 != 0 {
            let (query_path, rest) = take_uvarint(packed)?;
            let (query_start, rest) = take_uvarint(rest)?;
            let (query_length, rest) = take_uvarint(rest)?;
            query_start
                .checked_add(query_length)
                .ok_or("compact demand query location overflow")?;
            query = Some((query_path, query_start, query_length));
            packed = rest;
        }
        visit(header >> 1, start, length, query)?;
    }
    Ok(())
}

fn row_to_demand(
    path: &str,
    strings: &[String],
    flags: u64,
    start: u64,
    length: u64,
    query: Option<(u64, u64, u64)>,
) -> Result<EntityDemand, String> {
    let mut demand = plain_demand(
        Location {
            path: path.into(),
            start_byte: start,
            end_byte: start + length,
        },
        flags,
    );
    if let Some((query_path, query_start, query_length)) = query {
        demand.query_location = Some(Location {
            path: lookup(strings, query_path)?.into(),
            start_byte: query_start,
            end_byte: query_start + query_length,
        });
    }
    Ok(demand)
}

/// Expands one group on its own (Go: `CompactDemandsV3{Groups: {group}}.Expand`):
/// the whole group is shape-checked before anything else.
pub fn expand_compact_group(
    group: &CompactDemandGroup,
    strings: &[String],
) -> Result<Vec<EntityDemand>, String> {
    let (count, _) = compact_demand_shape(std::slice::from_ref(group))?;
    let path = lookup(strings, group.0)?;
    let mut demands = Vec::with_capacity(count);
    visit_group(group, strings, |flags, start, length, query| {
        demands.push(row_to_demand(path, strings, flags, start, length, query)?);
        Ok(())
    })?;
    Ok(demands)
}

/// Appends only the rows carrying `required_flag`, without expanding the
/// others (Go: `appendCompactDemandsWithFlag`).
pub fn append_compact_demands_with_flag(
    target: &mut Vec<EntityDemand>,
    group: &CompactDemandGroup,
    strings: &[String],
    required_flag: u64,
) -> Result<(), String> {
    let path = lookup(strings, group.0)?;
    visit_group(group, strings, |flags, start, length, query| {
        if flags & required_flag != 0 {
            target.push(row_to_demand(path, strings, flags, start, length, query)?);
        }
        Ok(())
    })
}

/// Converts the compact demand snapshot back into plain demands, in group
/// order.
pub fn expand_compact_demands(compact: &CompactDemands) -> Result<Vec<EntityDemand>, String> {
    let (count, _) = compact_demand_shape(&compact.groups)?;
    let mut demands = Vec::with_capacity(count);
    for group in &compact.groups {
        let path = lookup(&compact.strings, group.0)?;
        visit_group(group, &compact.strings, |flags, start, length, query| {
            demands.push(row_to_demand(
                path,
                &compact.strings,
                flags,
                start,
                length,
                query,
            )?);
            Ok(())
        })?;
    }
    Ok(demands)
}
