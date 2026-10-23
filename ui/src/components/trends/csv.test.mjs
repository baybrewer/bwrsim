import { test } from "node:test";
import assert from "node:assert/strict";
import { buildCsv, csvField, csvFileName } from "./csv.ts";

test("csv rows: time columns, fixed digits, empty for missing", () => {
  const text = buildCsv([0, 61.5], [null, null], [
    { header: "Rx pressure [MPa]", digits: 3, values: [7.1, null] },
    { header: "Level, NR [cm]", digits: 1, values: [12.34, 15] },
  ]);
  assert.equal(text, 'sim_time_s,sim_time,interval_end_s,Rx pressure [MPa],"Level, NR [cm]"\r\n0,00:00:00,,7.100,12.3\r\n61.5,00:01:01,,,15.0\r\n');
});

test("csv interval rows carry their end bound", () => {
  const text = buildCsv([100], [200], [{ header: "x", digits: 0, values: [199] }]);
  assert.equal(text, "sim_time_s,sim_time,interval_end_s,x\r\n100,00:01:40,200,199\r\n");
});

test("quoting and file names", () => {
  assert.equal(csvField('a b'), 'a b');
  assert.equal(csvField('a "b"'), '"a ""b"""');
  assert.equal(csvField('a,"b"'), '"a,""b"""');
  assert.equal(csvFileName(900, 1500), "bwrsim-trends_001500-002500.csv");
});
