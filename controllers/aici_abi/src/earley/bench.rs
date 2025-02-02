use anyhow::Result;
use quick_protobuf::MessageRead;
use rustc_hash::FxHashSet;

use super::{guidance, ByteSet, Grammar, Parser};
use crate::earley::parser::ParseResult;

pub fn earley_grm_from_guidance(bytes: &[u8]) -> Result<Grammar> {
    let mut reader = quick_protobuf::BytesReader::from_bytes(bytes);
    let gg = guidance::Grammar::from_reader(&mut reader, bytes).unwrap();
    let mut grm = Grammar::new();

    let symbols = gg
        .nodes
        .iter()
        .map(|n| match &n.function_type {
            guidance::mod_GrammarFunction::OneOffunction_type::join(n) => grm.fresh_symbol(&n.name),
            guidance::mod_GrammarFunction::OneOffunction_type::select(n) => {
                grm.fresh_symbol(&n.name)
            }
            guidance::mod_GrammarFunction::OneOffunction_type::byte(n) => {
                assert!(n.byte.len() == 1);
                grm.terminal(&ByteSet::from_range(n.byte[0], n.byte[0]))
            }
            guidance::mod_GrammarFunction::OneOffunction_type::byte_range(n) => {
                assert!(n.byte_range.len() == 2);
                grm.terminal(&ByteSet::from_range(n.byte_range[0], n.byte_range[1]))
            }
            guidance::mod_GrammarFunction::OneOffunction_type::model_variable(n) => {
                grm.fresh_symbol(&n.name)
            }
            guidance::mod_GrammarFunction::OneOffunction_type::None => {
                panic!("None function type in guidance::Grammar")
            }
        })
        .collect::<Vec<_>>();

    let set = FxHashSet::from_iter(symbols.iter());
    assert!(set.len() == symbols.len(), "duplicate symbols");

    for (n, sym) in gg.nodes.iter().zip(symbols.iter()) {
        let lhs = *sym;
        match &n.function_type {
            guidance::mod_GrammarFunction::OneOffunction_type::join(n) => {
                if n.nullable {
                    //println!("nullable join: {:?}", n.name);
                }
                let rhs = n.values.iter().map(|idx| symbols[*idx as usize]).collect();
                grm.add_rule(lhs, rhs);
            }
            guidance::mod_GrammarFunction::OneOffunction_type::select(n) => {
                if n.nullable {
                    // println!("nullable sel: {:?} {:?}", n.name, n.values);
                    grm.add_rule(lhs, vec![]);
                }
                for v in &n.values {
                    grm.add_rule(lhs, vec![symbols[*v as usize]]);
                }
            }
            guidance::mod_GrammarFunction::OneOffunction_type::byte(_) => {}
            guidance::mod_GrammarFunction::OneOffunction_type::byte_range(_) => {}
            guidance::mod_GrammarFunction::OneOffunction_type::model_variable(n) => {
                // eos_token, bos_token etc
                panic!("model_variable not implemented yet ({:?})", n.name);
            }
            guidance::mod_GrammarFunction::OneOffunction_type::None => panic!("???"),
        }
    }

    grm.add_rule(grm.start(), vec![symbols[0]]);

    Ok(grm)
}

pub fn earley_test(trie: crate::toktree::TokTrie) {
    let g_bytes = include_bytes!("../../grammars/json0.guidance");
    let cfg = earley_grm_from_guidance(g_bytes).unwrap();
    // println!("cfg0: {:?}", cfg);
    let cfg = cfg.optimize();
    println!("cfg: {:?}", cfg);

    let input = r#"{"name":"Joe","info":{"foo":10,"bar":"20"}}"#.as_bytes();

    let toks = trie.greedy_tokenize(input);
    println!("tokens: {:?}", toks.len());

    let grm = cfg.compile();

    let mut parser = Parser::new(grm.clone());
    let mut last_res = ParseResult::Reject;
    for b in input {
        last_res = parser.scan(*b);
        if last_res == ParseResult::Reject {
            println!("reject");
            break;
        }
    }
    if last_res != ParseResult::Accept {
        println!("final non-accept");
    }

    const COLLECT_TIMES: bool = false;
    const NUM_REP: usize = if COLLECT_TIMES { 5 } else { 500 };
    let mut durations = vec![];
    let mut durations_us = vec![];
    println!("start!");

    let num_tok = 4;

    for _ in 0..NUM_REP {
        let mut line = 1;
        let mut vob = trie.alloc_token_set();

        parser = Parser::new(grm.clone());
        let mut times = vec![];

        #[cfg(not(target_arch = "wasm32"))]
        let t0 = std::time::Instant::now();

        for (idx, tok) in toks.iter().take(num_tok).enumerate() {
            let tok = *tok;
            let tt = std::time::Instant::now();
            trie.compute_bias(&mut parser, &mut vob);
            if idx == num_tok - 1 {
                durations_us.push(tt.elapsed().as_micros() as u64);
            }
            // parser.print_stats();
            if !vob.is_allowed(tok) {
                println!("reject, line={}, tok={:?}", line, trie.token_str(tok));
                panic!();
            }
            for b in trie.token(tok) {
                if *b == b'\n' {
                    line += 1;
                }
            }
            // println!(
            //     "TOK: {} ===> {}",
            //     trie.token_dbg(tok),
            //     trie.token_set_dbg(&vob)
            // );
            trie.append_token(&mut parser, tok);
            if COLLECT_TIMES {
                times.push(tt.elapsed().as_micros() as u32);
            }
        }

        durations.push(t0.elapsed().as_micros() as u64);

        if COLLECT_TIMES {
            println!("times: {:?}", times);
        }
    }

    durations.sort();
    durations_us.sort();

    let min_us = *durations_us.iter().min().unwrap();
    // println!("min_time_us: {:?}", min_us);
    // for ~5ms 0.1ms is the precision we expect
    println!("min_time_ms: {:.1}", min_us as f64 / 1000.0);
}
