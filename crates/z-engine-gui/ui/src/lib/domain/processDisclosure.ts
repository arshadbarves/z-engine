/** Open state of the process disclosure while a turn streams.
 *
 *  `autoOpened` records whether streaming — rather than the reader — owns the
 *  currently open panel. Only an auto-opened panel may close itself again, so
 *  a panel the reader opened before or during the run stays open. */
export interface ProcessDisclosure {
  expanded: boolean;
  autoOpened: boolean;
  preRunExpanded: boolean;
}

export function beginProcessRun(state: ProcessDisclosure): ProcessDisclosure {
  return {
    expanded: true,
    autoOpened: !state.expanded,
    preRunExpanded: state.expanded,
  };
}

export function endProcessRun(state: ProcessDisclosure): ProcessDisclosure {
  const expanded = state.autoOpened ? state.preRunExpanded : state.expanded;
  return { expanded, autoOpened: false, preRunExpanded: expanded };
}

export function transitionProcessRun(
  state: ProcessDisclosure,
  wasRunning: boolean,
  isRunning: boolean,
): ProcessDisclosure {
  if (isRunning && !wasRunning) return beginProcessRun(state);
  if (!isRunning && wasRunning) return endProcessRun(state);
  return state;
}

export function toggleProcessDisclosure(state: ProcessDisclosure): ProcessDisclosure {
  return { ...state, expanded: !state.expanded, autoOpened: false };
}
