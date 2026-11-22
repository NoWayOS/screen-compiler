// Synthetic Screen Compiler fixture: a generic form screen with the constructs
// form screens need, most needed first: conditional rendering, text from a
// prop between literal text (a template slot), a length limit and an input
// mode on a text input, a checkbox and a radio group; the one screen mixing a
// template slot, a plain slot in a group and an either. Written for Screen
// Compiler's tests; not taken from any product.
import { Button, Checkbox, Column, RadioGroup, Screen, Text, TextInput } from "nowayos:kit";

type BookingProps = {
  phoneError: string;
  placesLeft: number;
  full: boolean;
  onBook: () => void;
};

export function BookingForm(props: BookingProps) {
  return (
    <Screen>
      <Column gap={8}>
        <Text id="heading">Book a place</Text>
        {props.full ? <Text>This session is full.</Text> : <Text>Places are available.</Text>}
        <Text>Places left: {props.placesLeft}</Text>
        <Text>Phone number</Text>
        <TextInput id="phone" placeholder="Digits only" inputMode="tel" maxLength={15} />
        {props.phoneError && <Text>{props.phoneError}</Text>}
        <Checkbox id="reminder" label="Remind me the day before" />
        <RadioGroup id="slot" options={["Morning", "Afternoon", "Evening"]} />
        <Button id="book" onPress={props.onBook}>
          Book
        </Button>
      </Column>
    </Screen>
  );
}
