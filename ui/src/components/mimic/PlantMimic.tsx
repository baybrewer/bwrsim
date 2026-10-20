import type { Command, Snapshot } from "../../sim/types";
import { clamp, fx } from "../../util/format";
import { Pipe, Pump, Tag, Valve } from "./symbols";

// Vessel geometry (SVG units). Level scale: y = TAF_Y - metres * PX_PER_M.
const VX0 = 200;
const VX1 = 320;
const VTOP = 40;
const VBOT = 582;
const SHROUD0 = 222;
const SHROUD1 = 298;
const TAF_Y = 470;
const PX_PER_M = 20;
const BAF_Y = TAF_Y + 3.8 * PX_PER_M;
const CHIMNEY_TOP_Y = 300;
const yOfLevel = (m: number) => TAF_Y - clamp(m, -5.5, 20) * PX_PER_M;

const vesselPath = `M ${VX0} ${VTOP + 40} A 60 40 0 0 1 ${VX1} ${VTOP + 40} L ${VX1} ${VBOT - 30} A 60 30 0 0 1 ${VX0} ${VBOT - 30} Z`;

export function PlantMimic(props: { s: Snapshot; send: (c: Command) => void }) {
  const { s, send } = props;
  const v = s.vessel;
  const st = s.steam;
  const tb = s.turbine;
  const cd = s.condenser;
  const fw = s.feedwater;
  const levelY = yOfLevel(v.levelAboveTafM);
  const avgWithdraw = s.rods.positions.reduce((a, p) => a + p, 0) / (48 * Math.max(1, s.rods.positions.length));
  const rodTipY = TAF_Y + (BAF_Y - TAF_Y) * avgWithdraw;
  const circDur = v.coreFlowKgs > 40 ? clamp(1500 / v.coreFlowKgs, 0.4, 8) : 0;
  const steamFlowing = v.steamFlowKgs > 1;
  const feedFlowing = v.feedFlowKgs > 1;
  const hotwellY = 360 - (clamp(cd.hotwellLevelPct, 0, 100) / 100) * 40;
  const nwlY = yOfLevel(9.5);

  return (
    <svg className="mimic" viewBox="0 0 1000 600" preserveAspectRatio="xMidYMid meet">
      <defs>
        <clipPath id="vessel-clip">
          <path d={vesselPath} />
        </clipPath>
        <marker id="arrow" viewBox="0 0 10 10" refX="5" refY="5" markerWidth="5" markerHeight="5" orient="auto-start-reverse">
          <path d="M 0 0 L 10 5 L 0 10 z" className="arrow-head" />
        </marker>
        <pattern id="fuel" width="6" height="10" patternUnits="userSpaceOnUse">
          <rect width="6" height="10" className="fuel-bg" />
          <line x1="3" y1="0" x2="3" y2="10" className="fuel-pin" />
        </pattern>
      </defs>

      {/* ---------------- Isolation condensers ---------------- */}
      <g className="ic">
        <rect x={12} y={48} width={130} height={170} className="pool" />
        {s.ic[0] && (
          <rect
            x={12}
            y={48 + 170 * (1 - clamp(s.ic[0].poolLevelPct, 0, 100) / 100)}
            width={130}
            height={170 * (clamp(s.ic[0].poolLevelPct, 0, 100) / 100)}
            className="pool-water"
          />
        )}
        <Tag x={77} y={64} text={`IC POOL ${s.ic[0] ? fx(s.ic[0].poolTempC, 0) : "--"}°C`} />
        <Pipe d={`M 240 ${VTOP + 4} L 240 28 L 40 28`} kind="steam" />
        {s.ic.map((ic, i) => {
          const x = 40 + i * 36;
          const yRet = 262 - i * 12;
          const zig = Array.from({ length: 6 }, (_, k) => `L ${x + (k % 2 ? -8 : 8)} ${92 + k * 14}`).join(" ");
          return (
            <g key={i}>
              <Pipe d={`M ${x} 28 L ${x} 80 ${zig} L ${x} 180`} kind="steam" flowing={ic.valvePos > 0.05} />
              <Pipe d={`M ${x} 180 L ${x} ${yRet} L ${VX0} ${yRet}`} kind="water" flowing={ic.valvePos > 0.05} />
              <Valve
                x={160}
                y={yRet}
                pos={ic.valvePos}
                size={6}
                onClick={() => send({ type: "setIcTrain", index: i, open: !ic.commandOpen })}
                title={`IC train ${String.fromCharCode(65 + i)} return valve — click to ${ic.commandOpen ? "close" : "open"}`}
              />
              <Tag x={x} y={200} text={`${String.fromCharCode(65 + i)} ${fx(ic.heatMw, 0)}`} tone={ic.heatMw > 0.5 ? undefined : "dim"} />
            </g>
          );
        })}
        <text x={77} y={287} textAnchor="middle" className="m-label">
          IC RETURN VALVES (MW)
        </text>
      </g>

      {/* ---------------- Reactor vessel ---------------- */}
      <g className="rpv">
        <g clipPath="url(#vessel-clip)">
          <rect x={VX0} y={VTOP} width={VX1 - VX0} height={VBOT - VTOP} className="steam-space" />
          <rect x={VX0} y={levelY} width={VX1 - VX0} height={VBOT - levelY + 5} className="water" />
          {/* voids */}
          <rect x={SHROUD0} y={TAF_Y} width={SHROUD1 - SHROUD0} height={BAF_Y - TAF_Y} className="void" style={{ opacity: v.coreAvgVoid * 0.9 }} />
          <rect
            x={SHROUD0}
            y={Math.max(levelY, CHIMNEY_TOP_Y)}
            width={SHROUD1 - SHROUD0}
            height={Math.max(0, TAF_Y - Math.max(levelY, CHIMNEY_TOP_Y))}
            className="void"
            style={{ opacity: v.chimneyVoid * 0.9 }}
          />
        </g>
        {/* core */}
        <rect x={SHROUD0 + 4} y={TAF_Y} width={SHROUD1 - SHROUD0 - 8} height={BAF_Y - TAF_Y} fill="url(#fuel)" className="core" style={{ fillOpacity: 0.55 }} />
        <rect
          x={SHROUD0 + 4}
          y={TAF_Y}
          width={SHROUD1 - SHROUD0 - 8}
          height={BAF_Y - TAF_Y}
          className="core-glow"
          style={{ opacity: clamp(s.neutronics.thermalPowerMw / 870, 0, 1.2) * 0.55 }}
        />
        {/* control rods from below */}
        {[0, 1, 2, 3, 4].map((k) => (
          <rect key={k} x={SHROUD0 + 10 + k * 14} y={rodTipY} width={4} height={VBOT - 20 - rodTipY} className="rod-blade" />
        ))}
        {/* shroud + chimney */}
        <path d={`M ${SHROUD0} ${CHIMNEY_TOP_Y} L ${SHROUD0} ${BAF_Y + 8} M ${SHROUD1} ${CHIMNEY_TOP_Y} L ${SHROUD1} ${BAF_Y + 8}`} className="shroud" />
        {/* separators / dryer */}
        <rect x={SHROUD0} y={175} width={SHROUD1 - SHROUD0} height={18} className="dryer" />
        <text x={(VX0 + VX1) / 2} y={188} textAnchor="middle" className="m-tiny">
          DRYER
        </text>
        <text x={(VX0 + VX1) / 2} y={385} textAnchor="middle" className="m-tiny">
          CHIMNEY
        </text>
        <text x={(VX0 + VX1) / 2} y={512} textAnchor="middle" className="m-tiny">
          CORE
        </text>
        {/* level marks */}
        <line x1={VX0 - 6} x2={VX0} y1={nwlY} y2={nwlY} className="mark" />
        <text x={VX0 - 8} y={nwlY + 3} textAnchor="end" className="m-tiny">
          NWL
        </text>
        <line x1={VX0 - 6} x2={VX0} y1={TAF_Y} y2={TAF_Y} className="mark" />
        <text x={VX0 - 8} y={TAF_Y + 3} textAnchor="end" className="m-tiny">
          TAF
        </text>
        <line x1={VX0 + 2} x2={VX1 - 2} y1={levelY} y2={levelY} className="level-line" />
        {/* natural circulation */}
        {circDur > 0 && (
          <g className="circ" style={{ animationDuration: `${circDur}s` }}>
            <path d={`M 260 ${BAF_Y - 4} L 260 ${CHIMNEY_TOP_Y + 20}`} markerEnd="url(#arrow)" />
            <path d={`M 211 ${CHIMNEY_TOP_Y + 20} L 211 ${BAF_Y + 10}`} markerEnd="url(#arrow)" />
            <path d={`M 309 ${CHIMNEY_TOP_Y + 20} L 309 ${BAF_Y + 10}`} markerEnd="url(#arrow)" />
          </g>
        )}
        <path d={vesselPath} className="vessel-wall" />
        {/* head vent */}
        <Pipe d={`M 270 ${VTOP + 2} L 270 14 L 300 14`} kind="steam" flowing={v.headVentOpen} />
        <Valve
          x={286}
          y={14}
          pos={v.headVentOpen ? 1 : 0}
          size={6}
          onClick={() => send({ type: "setHeadVent", open: !v.headVentOpen })}
          label="VENT"
          labelPos="right"
          title={`Head vent — click to ${v.headVentOpen ? "close" : "open"}`}
        />
        <text x={(VX0 + VX1) / 2} y={VBOT + 14} textAnchor="middle" className="m-title">
          RPV
        </text>
      </g>

      {/* RWCU reject */}
      <g>
        <Pipe d={`M ${VX0} 534 L 190 534`} kind="water" flowing={fw.rwcuRejectKgs > 0.05} />
        <rect x={110} y={520} width={80} height={28} rx={3} className="condenser" />
        <text x={150} y={538} textAnchor="middle" className="m-label">
          RWCU
        </text>
        <Tag x={150} y={512} text={`REJ ${fx(fw.rwcuRejectKgs, 1)} kg/s`} tone={fw.rwcuRejectKgs > 0.05 ? undefined : "dim"} />
      </g>

      {/* vessel readouts */}
      <g>
        <Tag x={330} y={150} anchor="start" text={`P ${fx(v.pressureMpa, 2)} MPa`} tone={v.pressureMpa > 7.5 ? "alarm" : undefined} />
        <Tag x={330} y={166} anchor="start" text={`L ${fx(v.levelNrCm, 0)} cm NR`} tone={Math.abs(v.levelNrCm) > 60 ? "warn" : undefined} />
        <Tag x={330} y={182} anchor="start" text={`T ${fx(v.coolantTempC, 0)} °C`} />
        <Tag x={330} y={198} anchor="start" text={`Wc ${fx(v.coreFlowKgs, 0)} kg/s`} />
        <Tag x={330} y={214} anchor="start" text={`α ${fx(v.coreAvgVoid * 100, 0)}/${fx(v.chimneyVoid * 100, 0)} %`} />
        <Tag x={330} y={230} anchor="start" text={`${fx(s.neutronics.thermalPowerMw, 0)} MWt`} />
      </g>

      {/* ---------------- Main steam ---------------- */}
      {[0, 1].map((i) => {
        const y = 82 + i * 22;
        const pos = st.msivPos[i] ?? 0;
        return (
          <g key={i}>
            <Pipe d={`M ${VX1 - 2} ${y} L 430 ${y} L 440 93`} kind="steam" flowing={steamFlowing && pos > 0.05} />
            <Valve
              x={378}
              y={y}
              pos={pos}
              label={i === 0 ? `MSIV ${String.fromCharCode(65 + i)}` : undefined}
              labelPos="above"
              onClick={() => send({ type: "setMsiv", line: i, open: !(st.msivCommandOpen[i] ?? false) })}
              title={`MSIV line ${String.fromCharCode(65 + i)} — click to ${st.msivCommandOpen[i] ? "close" : "open"}`}
            />
          </g>
        );
      })}
      <text x={378} y={130} textAnchor="middle" className="m-label">
        MSIV B
      </text>
      <Pipe d="M 440 93 L 650 93" kind="steam" flowing={st.steamToTurbineKgs > 1} />
      <Tag x={500} y={78} text={`${fx(v.steamFlowKgs, 0)} kg/s`} />
      <Valve x={612} y={93} pos={st.tcvPos} label={`TCV ${fx(st.tcvPos * 100, 0)}%`} labelPos="above" title="Turbine control valve" />

      {/* bypass */}
      <Pipe d="M 470 93 L 470 330 L 600 330" kind="steam" flowing={st.steamToBypassKgs > 1} />
      <Valve x={470} y={215} pos={st.bypassPos} vertical label={`BYP ${fx(st.bypassPos * 100, 0)}%`} labelPos="right" title="Turbine bypass valve" />
      <Tag x={478} y={250} anchor="start" text={`${fx(st.steamToBypassKgs, 0)} kg/s`} tone={st.steamToBypassKgs > 1 ? undefined : "dim"} />

      {/* ---------------- Turbine-generator ---------------- */}
      <path d="M 650 78 L 650 108 L 790 135 L 790 51 Z" className={`turbine${tb.tripped ? " tripped" : " latched"}`} />
      <text x={720} y={97} textAnchor="middle" className="m-title">
        TURBINE
      </text>
      <line x1={790} y1={93} x2={815} y2={93} className="shaft" />
      <rect x={815} y={70} width={66} height={46} rx={4} className={`generator${tb.breakerClosed ? " online" : ""}`} />
      <text x={848} y={97} textAnchor="middle" className="m-title">
        GEN
      </text>
      <line x1={881} y1={93} x2={900} y2={93} className="bus" />
      <g className={`breaker${tb.breakerClosed ? " closed" : ""}`}>
        <rect x={900} y={85} width={16} height={16} />
      </g>
      <line x1={916} y1={93} x2={985} y2={93} className={`bus${tb.breakerClosed ? " live" : ""}`} />
      <text x={985} y={85} textAnchor="end" className="m-label">
        GRID
      </text>
      <Tag x={848} y={58} text={`${fx(tb.grossMwe, 0)} MWe`} />
      <Tag x={720} y={150} text={`${fx(tb.speedRpm, 0)} rpm`} tone={tb.tripped ? "dim" : undefined} />
      <Tag x={950} y={112} text={tb.breakerClosed ? "52G CLOSED" : "52G OPEN"} tone={tb.breakerClosed ? undefined : "dim"} />
      <Pipe d="M 760 128 L 760 250" kind="steam" flowing={st.steamToTurbineKgs > 1} />

      {/* ---------------- Condenser ---------------- */}
      <rect x={600} y={250} width={260} height={110} rx={4} className="condenser" />
      <rect x={602} y={hotwellY} width={256} height={358 - hotwellY} className="water" />
      <text x={730} y={272} textAnchor="middle" className="m-title">
        MAIN CONDENSER
      </text>
      <Tag x={730} y={292} text={`${fx(cd.pressureKpa, 1)} kPa`} tone={cd.pressureKpa > 25 ? "alarm" : cd.pressureKpa > 15 ? "warn" : undefined} />
      <Tag x={730} y={310} text={`HW ${fx(cd.hotwellLevelPct, 0)}%`} />
      {/* CW */}
      {cd.cwPumps.map((on, i) => {
        const y = 285 + i * 40;
        return (
          <g key={i}>
            <Pipe d={`M 860 ${y} L 930 ${y}`} kind="cw" flowing={on} />
            <Pump
              x={940}
              y={y}
              running={on}
              dir="left"
              label={`CW ${String.fromCharCode(65 + i)}`}
              onClick={() => send({ type: "setCwPump", index: i, on: !on })}
            />
          </g>
        );
      })}
      <g className={`vac${cd.vacuumPumpOn ? " running" : ""}`}>
        <Pump x={830} y={225} r={8} running={cd.vacuumPumpOn} dir="up" onClick={() => send({ type: "setVacuumPump", on: !cd.vacuumPumpOn })} />
        <text x={843} y={229} className="m-label">
          VAC
        </text>
        <Pipe d="M 830 233 L 830 250" kind="steam" />
      </g>

      {/* ---------------- Condensate / feedwater train ---------------- */}
      <Pipe d="M 730 360 L 730 395 L 680 395 L 680 410 M 730 395 L 790 395 L 790 410" kind="water" flowing={feedFlowing} />
      {fw.condensatePumps.map((on, i) => {
        const x = 680 + i * 110;
        return (
          <Pump
            key={i}
            x={x}
            y={422}
            running={on}
            dir="left"
            label={`CP ${String.fromCharCode(65 + i)}`}
            onClick={() => send({ type: "setCondensatePump", index: i, on: !on })}
          />
        );
      })}
      <Pipe d="M 680 434 L 680 470 M 790 434 L 790 470 L 425 470" kind="water" flowing={feedFlowing} />
      {[0, 1].map((k) => (
        <g key={k} className={`heater${fw.heatersInService ? " in" : ""}`}>
          <rect x={590 - k * 90} y={458} width={60} height={24} rx={12} />
          <text x={620 - k * 90} y={474} textAnchor="middle" className="m-tiny">
            HTR {k + 1}
          </text>
        </g>
      ))}
      <Tag x={545} y={500} text={`${fx(fw.feedTempC, 0)} °C ${fw.heatersInService ? "HTR IN" : "HTR OUT"}`} />

      {/* feed pumps: suction header x=425, discharge header x=375 */}
      <Pipe d="M 425 410 L 425 530" kind="water" flowing={feedFlowing} />
      <Pipe d="M 375 410 L 375 530" kind="water" flowing={feedFlowing} />
      {fw.pumps.map((p, i) => {
        const y = 410 + i * 60;
        return (
          <g key={i}>
            <Pipe d={`M 425 ${y} L 375 ${y}`} kind="water" flowing={p.running && p.flowKgs > 1} />
            <Pump
              x={400}
              y={y}
              running={p.running}
              dir="left"
              onClick={() => send({ type: "setFeedPump", index: i, on: !p.running })}
              label={`FWP ${String.fromCharCode(65 + i)}`}
            />
            <Tag x={368} y={y + 4} anchor="end" text={fx(p.flowKgs, 0)} tone={p.running ? undefined : "dim"} />
          </g>
        );
      })}
      <Pipe d={`M 375 410 L 375 290 L ${VX1 - 2} 290`} kind="water" flowing={feedFlowing} />
      <Valve x={375} y={350} pos={fw.fcvPos} vertical label={`FCV ${fx(fw.fcvPos * 100, 0)}%`} labelPos="right" title="Feedwater control valve" />
      <Tag x={386} y={318} anchor="start" text={`${fx(v.feedFlowKgs, 0)} kg/s`} />
      <text x={400} y={560} textAnchor="middle" className="m-label">
        FEEDWATER PUMPS
      </text>

      {/* legend */}
      <g className="mimic-legend" transform="translate(600 540)">
        <Valve x={8} y={0} pos={1} size={6} />
        <text x={20} y={4} className="m-tiny">
          OPEN
        </text>
        <Valve x={68} y={0} pos={0} size={6} />
        <text x={80} y={4} className="m-tiny">
          CLOSED
        </text>
        <Pump x={140} y={0} r={7} running />
        <text x={152} y={4} className="m-tiny">
          RUNNING
        </text>
        <Pump x={214} y={0} r={7} running={false} />
        <text x={226} y={4} className="m-tiny">
          STOPPED
        </text>
        <text x={0} y={24} className="m-tiny">
          Click MSIVs, IC valves, pumps, head vent to operate.
        </text>
      </g>
    </svg>
  );
}
