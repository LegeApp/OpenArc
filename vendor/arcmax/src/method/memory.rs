use crate::codec::MemoryUsage;
use crate::method::Method;

pub fn estimate(method: &Method) -> MemoryUsage {
    match method {
        Method::Store => MemoryUsage::default(),
        Method::Srep(options) => MemoryUsage {
            working_bytes: (options.block_size + options.dict_size) as u64,
            ..MemoryUsage::default()
        },
        Method::Pipeline(stages) => {
            stages
                .iter()
                .map(estimate)
                .fold(MemoryUsage::default(), |mut total, usage| {
                    total.read_bytes += usage.read_bytes;
                    total.write_bytes += usage.write_bytes;
                    total.working_bytes += usage.working_bytes;
                    total
                })
        }
        _ => MemoryUsage::default(),
    }
}
