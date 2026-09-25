import { nextStep, prevStep, type OnboardingStep } from "../domain/onboarding";

/** First-run setup: whether it is showing, the step, and the project added during it. */
class OnboardingStore {
  active = $state(false);
  step = $state<OnboardingStep>("welcome");
  project = $state<string | null>(null);

  start() {
    this.active = true;
    this.step = "welcome";
    this.project = null;
  }

  next() {
    this.step = nextStep(this.step);
  }

  back() {
    this.step = prevStep(this.step);
  }

  finish() {
    this.active = false;
  }
}

export const onboarding = new OnboardingStore();
