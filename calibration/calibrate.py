#!/usr/bin/env python3
"""Reproduce the sedan thermal RC calibration without third-party packages.

Source: Ramirez-Dolores et al., Mendeley Data v1,
DOI 10.17632/8mfgd8w9rg.1, CC BY 4.0.
The script verifies the publisher SHA-256 before reading experiment d, writes the
normalized observations, fits on the first 70%, and reports a recursive 30%
holdout. It never silently accepts another workbook revision.
"""
from __future__ import annotations

import csv
import hashlib
import json
import math
import re
from pathlib import Path
from xml.etree import ElementTree as ET
from zipfile import ZipFile

ROOT = Path(__file__).resolve().parent
SOURCE = ROOT / "source" / "thermal-cabin-database.xlsx"
DATASET = ROOT / "datasets" / "mendeley-sedan-experiment-d.csv"
PROFILE = ROOT / "profiles" / "mendeley-sedan-v1.json"
EXPECTED_SHA256 = "9075e138317faa93be66891af8173dc9070e3782105d2f40f9f6f2273e89e777"
DOI = "10.17632/8mfgd8w9rg.1"
NS = {"m": "http://schemas.openxmlformats.org/spreadsheetml/2006/main"}


def workbook_rows() -> list[tuple[int, float, float]]:
    digest = hashlib.sha256(SOURCE.read_bytes()).hexdigest()
    if digest != EXPECTED_SHA256:
        raise SystemExit(f"source SHA-256 mismatch: {digest}")
    with ZipFile(SOURCE) as archive:
        sheet = ET.fromstring(archive.read("xl/worksheets/sheet4.xml"))
    rows: list[tuple[int, float, float]] = []
    for row in sheet.findall(".//m:sheetData/m:row", NS):
        if int(row.attrib["r"]) < 8:
            continue
        values: dict[str, float] = {}
        for cell in row.findall("m:c", NS):
            value = cell.find("m:v", NS)
            if value is not None:
                column = re.match(r"[A-Z]+", cell.attrib["r"])
                if column:
                    values[column.group()] = float(value.text)
        if all(column in values for column in ("A", "B", "H")):
            rows.append((int(values["A"]), values["B"], values["H"]))
    if len(rows) != 1302:
        raise SystemExit(f"expected 1302 experiment-d observations, found {len(rows)}")
    return rows


def metrics(predicted: list[float], actual: list[float]) -> dict[str, float]:
    errors = [prediction - observation for prediction, observation in zip(predicted, actual)]
    return {
        "rmseC": math.sqrt(sum(error * error for error in errors) / len(errors)),
        "maeC": sum(abs(error) for error in errors) / len(errors),
        "maxAbsErrorC": max(abs(error) for error in errors),
    }


def main() -> None:
    rows = workbook_rows()
    sample_period_s = 5.0 * 3600.0 / (len(rows) - 1)
    training_count = int(len(rows) * 0.7)
    x: list[float] = []
    y: list[float] = []
    for (_, outside, cabin), (_, _, next_cabin) in zip(
        rows[: training_count - 1], rows[1:training_count]
    ):
        x.append(outside - cabin)
        y.append(next_cabin - cabin)
    mean_x = sum(x) / len(x)
    mean_y = sum(y) / len(y)
    alpha = sum((a - mean_x) * (b - mean_y) for a, b in zip(x, y)) / sum(
        (a - mean_x) ** 2 for a in x
    )
    bias_c_per_step = mean_y - alpha * mean_x

    thermal_mass_j_per_k = 180_000.0
    ua_w_per_k = alpha * thermal_mass_j_per_k / sample_period_s
    mean_solar_load_w = bias_c_per_step * thermal_mass_j_per_k / sample_period_s

    one_step_predicted: list[float] = []
    one_step_actual: list[float] = []
    for (_, outside, cabin), (_, _, next_cabin) in zip(rows, rows[1:]):
        one_step_predicted.append(cabin + alpha * (outside - cabin) + bias_c_per_step)
        one_step_actual.append(next_cabin)

    recursive = rows[training_count][2]
    holdout_predicted: list[float] = []
    holdout_actual: list[float] = []
    for index in range(training_count, len(rows) - 1):
        outside = rows[index][1]
        recursive += alpha * (outside - recursive) + bias_c_per_step
        holdout_predicted.append(recursive)
        holdout_actual.append(rows[index + 1][2])

    DATASET.parent.mkdir(parents=True, exist_ok=True)
    with DATASET.open("w", newline="", encoding="utf-8") as handle:
        writer = csv.writer(handle, lineterminator="\n")
        writer.writerow(("measurementId", "elapsedSeconds", "outsideTemperatureC", "cabinSensorMedianC"))
        for measurement_id, outside, cabin in rows:
            writer.writerow((measurement_id, f"{(measurement_id - 1) * sample_period_s:.9f}", f"{outside:.9f}", f"{cabin:.9f}"))

    profile = {
        "schemaVersion": 1,
        "profileId": "mendeley-sedan-v1",
        "modelVersion": "cockpit-multiphysics-4",
        "source": {
            "doi": DOI,
            "version": 1,
            "license": "CC BY 4.0",
            "workbookSha256": EXPECTED_SHA256,
            "experiment": "d: closed parked sedan, five continuous hours",
            "observations": len(rows),
            "samplePeriodS": sample_period_s,
        },
        "fit": {
            "method": "ordinary-least-squares first-order thermal RC",
            "trainingObservations": training_count,
            "holdoutObservations": len(holdout_actual),
            "alphaPerSample": alpha,
            "biasCPerSample": bias_c_per_step,
            "thermalMassJPerK": thermal_mass_j_per_k,
            "uaWPerK": ua_w_per_k,
            "effectiveSolarLoadAtReferenceIrradianceW": mean_solar_load_w,
        },
        "validation": {
            "oneStepAll": metrics(one_step_predicted, one_step_actual),
            "recursiveHoldout": metrics(holdout_predicted, holdout_actual),
            "persistenceHoldout": metrics(
                [rows[training_count][2]] * len(holdout_actual), holdout_actual
            ),
            "accepted": metrics(holdout_predicted, holdout_actual)["rmseC"] < 2.1,
            "acceptanceThresholdRmseC": 2.1,
        },
        "empiricalBaselines": {
            "smokeOptics": {
                "source": "Mulholland and Croarkin, Specific extinction coefficient of flame generated smoke (2000)",
                "sourceUrl": "https://www.nist.gov/publications/comparisons-soot-volume-fraction-using-gravimetric-and-light-extinction-techniques-0",
                "measuredMeanM2PerG": 8.7,
                "expandedUncertainty95PctM2PerG": 1.1,
                "runtimeM2PerMg": 0.0087,
                "status": "experimentally anchored optical coefficient; vehicle source and deposition are not calibrated",
            },
            "vehicleSmokeDeposition": {
                "source": "Ott, Klepeis and Switzer, J Expo Sci Environ Epidemiol 18 (2008)",
                "doi": "10.1038/sj.jes.7500601",
                "vehicleAirChangeMeasurements": ">100 across four vehicles",
                "smokeDecayExperiments": 14,
                "depositionToAirChangeRatio": 1.3,
                "regressionR2": 0.82,
                "status": "vehicle smoke deposition law drives runtime; cigarette PM2.5 transfer to fire soot is an applicability boundary",
            },
            "parkedCabinInfiltration": {
                "source": "Knibbs, de Dear and Atkinson, Field study of air change and flow rate in six automobiles (2009)",
                "doi": "10.1111/j.1600-0668.2009.00593.x",
                "vehicles": 6,
                "measurements": ">200",
                "stationaryObservedAch": [0.0, 1.4],
                "runtimeAch": 0.25,
                "status": "runtime value is inside the real parked-vehicle envelope; pressure equalization is not fitted",
            },
            "cabinAbsolutePressure": {
                "source": "Teleszewski and Gladyszewska-Fiedoruk, Sensors 26(2), 2026",
                "doi": "10.3390/s26020469",
                "license": "CC BY 4.0",
                "measurementSeries": 15,
                "probeUncertaintyHpa": 1.1,
                "measuredAltitudeDomainM": [0.0, 1500.0],
                "meanSlopeHpaPerM": -0.112,
                "meanInterceptHpa": 1013.6,
                "status": "published land-vehicle pressure-altitude fit drives runtime; pressure rate was not measured",
            },
            "carbonMonoxidePhysiology": {
                "source": "Alter, Dayan and Fleminger, Toxics 14(6), 2026",
                "doi": "10.3390/toxics14060488",
                "license": "CC BY 4.0",
                "model": "CFK-derived MIL-STD-1472H integrated equation, activity level 2",
                "participantsEnrolled": 100,
                "fieldSetting": "fully enclosed armored motor vehicles with live-fire CO exposure",
                "activityLevel": 2,
                "aMin": 241.0,
                "bInverseMmhg": 1421.0,
                "affinityRatio": 218.0,
                "peakCohbRmsePct": 1.94,
                "correlationR": 0.61,
                "meanPredictionErrorPct": 0.86,
                "status": "externally field-validated exposure and recovery kinetics drive runtime",
            },
            "thermoregulation": {
                "validationProfileId": "human-heat-stress-validation-v1",
                "humidityExerciseSource": "Che Muhamed et al., Temperature 3(3), 2016",
                "humidityExerciseDoi": "10.1080/23328940.2016.1182669",
                "humidityExerciseLicense": "CC BY-NC",
                "humidityExerciseProtocol": "11 trained males, 60 minutes at 31 C, 70% VO2max and 23-71% RH",
                "passiveHeatSource": "Malcolm et al., Frontiers in Physiology 9:585, 2018",
                "passiveHeatDoi": "10.3389/fphys.2018.00585",
                "passiveHeatLicense": "CC BY 4.0",
                "passiveHeatProtocol": "41 males, randomized crossover, 60 minutes seated at 39.6 C/50.8% RH or 21.2 C/41.9% RH",
                "status": "directional core/skin and humidity-response gate plus 37.0/33.7 C resting constraint; evaporative parameters are explicit but not cohort-fitted",
            },
            "fullScaleVehicleFire": {
                "source": "NIST Fire Calorimetry Database, Vehicle2 (2024)",
                "databaseDoi": "10.18434/mds2-2314",
                "recordUrl": "https://www.nist.gov/el/fcd/design-fires-vehicles-pine-straw-bed/vehicle2",
                "profileId": "nist-vehicle2-combustion-v1",
                "csvSha256": "4957b94564cd338dca3098e849309e5ce442f3c8a5e6191375a42d92f2463a26",
                "status": "measured HRR and aggregate soot/CO yields drive runtime; closed-cabin transfer remains uncalibrated",
            },
        },
        "scope": {
            "calibrated": [
                "closed-cabin aggregate thermal response",
                "full-scale ICE-vehicle HRR trajectory and aggregate soot/CO yields",
            ],
            "externallyValidatedModels": [
                "CFK-derived MIL-STD-1472H AL2 COHb exposure and recovery kinetics",
                "vehicle smoke deposition k=1.3a mass-balance law",
            ],
            "experimentallyAnchoredNotFitted": [
                "smoke mass-specific optical extinction",
                "parked-cabin infiltration rate",
                "land-vehicle cabin pressure-altitude regression",
                "core/skin heat-strain and humidity-response direction",
            ],
            "physicsBasedNotDatasetCalibrated": [
                "inter-zone heat exchange",
                "water-vapour mass balance",
                "exterior-fire effluent transfer into the closed cabin",
                "inter-zone smoke transport and transfer of cigarette-PM deposition law to fire soot",
                "individualized CFK physiology outside the validated AL2 young-male cohort",
                "cabin pressure equalization",
                "two-node occupant thermoregulation parameters",
            ],
        },
    }
    PROFILE.parent.mkdir(parents=True, exist_ok=True)
    PROFILE.write_text(json.dumps(profile, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    print(
        f"source_sha256={EXPECTED_SHA256} observations={len(rows)} "
        f"holdout_rmse_c={profile['validation']['recursiveHoldout']['rmseC']:.6f} "
        f"persistence_rmse_c={profile['validation']['persistenceHoldout']['rmseC']:.6f} "
        f"accepted={profile['validation']['accepted']}"
    )


if __name__ == "__main__":
    main()
